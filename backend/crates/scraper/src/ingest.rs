use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

use anyhow::Context;
use chrono::NaiveDate;
use common::{Contract, Cpv, Currency, Entity, db::ContractDatabase, searchdb::SearchDatabase};
use log::{error, info, warn};
use serde::Deserialize;

/// The raw JSON record matching the PyArrow schema from the JSON export files.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonContract {
    pub idcontrato: String,
    #[serde(default)]
    pub n_anuncio: String,
    #[serde(default)]
    pub tipo_anuncio: String,
    #[serde(default)]
    pub idincm: String,
    #[serde(default)]
    pub tipo_contrato: Vec<String>,
    #[serde(default)]
    pub idprocedimento: String,
    #[serde(default)]
    pub tipoprocedimento: String,
    #[serde(default)]
    pub objecto_contrato: String,
    #[serde(default)]
    pub desc_contrato: String,
    #[serde(default)]
    pub adjudicante: Vec<String>,
    #[serde(default)]
    pub adjudicatarios: Vec<String>,
    #[serde(default)]
    pub data_publicacao: String,
    #[serde(default)]
    pub data_celebracao_contrato: String,
    pub preco_contratual: f64,
    #[serde(default)]
    pub cpv: Vec<String>,
    pub prazo_execucao: i64,
    #[serde(default)]
    pub local_execucao: Vec<String>,
    #[serde(default)]
    pub fundamentacao: String,
    #[serde(default)]
    pub procedimento_centralizado: String,
    #[serde(default)]
    pub num_acordo_quadro: String,
    #[serde(default)]
    pub descr_acordo_quadro: String,
    pub preco_base_procedimento: f64,
    #[serde(default)]
    pub data_decisao_adjudicacao: String,
    #[serde(default)]
    pub data_fecho_contrato: String,
    pub preco_total_efetivo: f64,
    #[serde(default)]
    pub regime: String,
    #[serde(default)]
    pub justif_n_reduc_escr_contrato: Vec<String>,
    #[serde(default)]
    pub tipo_fim_contrato: String,
    #[serde(default)]
    pub crit_materiais: String,
    #[serde(default, deserialize_with = "deserialize_nullable_vec")]
    pub concorrentes: Vec<String>,
    #[serde(default)]
    pub link_pecas_proc: String,
    #[serde(default)]
    pub observacoes: String,
    #[serde(default)]
    pub contrat_ecologico: String,
    pub ano: i64,
    #[serde(default)]
    pub fundament_ajuste_direto: String,
}

/// Deserialize `null` as an empty vec (some fields like `concorrentes` can be null or missing).
fn deserialize_nullable_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt: Option<Vec<String>> = Deserialize::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

/// Parse a "DD/MM/YYYY" date string into a NaiveDate.
fn parse_date(s: &str) -> Option<NaiveDate> {
    if s.is_empty() {
        return None;
    }
    NaiveDate::parse_from_str(s.trim(), "%d/%m/%Y").ok()
}

/// Parse a "NIF - Name" or "NIF-Name" string into an Entity.
/// Supports both " - " (space-dash-space) and "-" (dash without spaces) as separators.
/// " - " is tried first. If the NIF part is "-" or empty, it's treated as absent.
fn parse_entity(s: &str) -> Entity {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        let mut hasher = DefaultHasher::new();
        "".hash(&mut hasher);
        return Entity {
            id: hasher.finish(),
            nif: String::new(),
            description: String::new(),
        };
    }

    // Try " - " first, then fall back to "-" without spaces
    let (nif_part, name_part) = if let Some(idx) = trimmed.find(" - ") {
        (trimmed[..idx].trim(), trimmed[idx + 3..].trim())
    } else if let Some(idx) = trimmed.find('-') {
        (trimmed[..idx].trim(), trimmed[idx + 1..].trim())
    } else {
        // No separator at all — hash the whole string as id
        let mut hasher = DefaultHasher::new();
        trimmed.hash(&mut hasher);
        return Entity {
            id: hasher.finish(),
            nif: String::new(),
            description: trimmed.to_string(),
        };
    };

    // If NIF part is empty, just "-", or not numeric, treat as no NIF
    let nif_clean = if nif_part.is_empty() || nif_part == "-" {
        String::new()
    } else {
        nif_part.to_string()
    };

    let name = if name_part.is_empty() {
        trimmed.to_string()
    } else {
        name_part.to_string()
    };

    let id = if nif_clean.is_empty() {
        let mut hasher = DefaultHasher::new();
        name.hash(&mut hasher);
        hasher.finish()
    } else {
        nif_to_id(&nif_clean)
    };

    Entity {
        id,
        nif: nif_clean,
        description: name,
    }
}

/// Derive a numeric entity ID from a NIF string.
/// First tries to parse the NIF directly as an i64.
/// Falls back to hashing the NIF string (e.g. for non-numeric NIFs).
fn nif_to_id(nif: &str) -> u64 {
    nif.parse::<i64>()
        .ok()
        .map(|v| v as u64)
        .unwrap_or_else(|| {
            let mut hasher = DefaultHasher::new();
            nif.hash(&mut hasher);
            hasher.finish()
        })
}

/// Parse a CPV string in the format "code - designation".
fn parse_cpv(s: &str) -> Option<Cpv> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(idx) = trimmed.find(" - ") {
        let code = trimmed[..idx].trim().to_string();
        let designation = trimmed[idx + 3..].trim().to_string();
        if code.is_empty() && designation.is_empty() {
            return None;
        }
        Some(Cpv { code, designation })
    } else {
        // CPV is just a code, no designation
        if trimmed.is_empty() {
            None
        } else {
            Some(Cpv {
                code: trimmed.to_string(),
                designation: String::new(),
            })
        }
    }
}

/// Parse prices: euro float → cents (isize).
/// Multiply by 100 and round.
fn parse_euros(value: f64) -> Currency {
    Currency((value * 100.0).round() as isize)
}

/// Parse optional price: if 0.0, treat as None.
fn parse_optional_euros(value: f64) -> Option<Currency> {
    if value == 0.0 {
        None
    } else {
        Some(parse_euros(value))
    }
}

/// Join a list of strings with " | ".
fn join_list(list: &[String]) -> String {
    list.join(" | ")
}

/// None if empty string, Some otherwise.
fn none_if_empty(s: &str) -> Option<String> {
    if s.is_empty() || s == "NULL" {
        None
    } else {
        Some(s.to_string())
    }
}

impl TryFrom<JsonContract> for Contract {
    type Error = anyhow::Error;

    fn try_from(raw: JsonContract) -> Result<Self, Self::Error> {
        let id = raw
            .idcontrato
            .parse::<u64>()
            .context("Invalid idcontrato")?;

        let publication_date =
            parse_date(&raw.data_publicacao).context("Invalid dataPublicacao")?;

        let signing_date = parse_date(&raw.data_celebracao_contrato);
        let close_date = parse_date(&raw.data_fecho_contrato);
        let total_effective_price = parse_optional_euros(raw.preco_total_efetivo);

        let end_of_contract_type = none_if_empty(&raw.tipo_fim_contrato);
        let observations = none_if_empty(&raw.observacoes);
        let regime = none_if_empty(&raw.regime);
        let contract_status: Option<String> = None; // Not in JSON
        let description = none_if_empty(&raw.desc_contrato);

        let announcement_id = if raw.n_anuncio.is_empty() {
            None
        } else {
            raw.n_anuncio.parse::<usize>().ok()
        };

        let contracting_procedure_url = none_if_empty(&raw.link_pecas_proc);

        // Entities
        let contracting: Vec<Entity> = raw.adjudicante.iter().map(|s| parse_entity(s)).collect();
        let contracted: Vec<Entity> = raw
            .adjudicatarios
            .iter()
            .map(|s| parse_entity(s))
            .collect();
        let contestants: Vec<Entity> = raw.concorrentes.iter().map(|s| parse_entity(s)).collect();

        // CPVs
        let cpvs: Vec<Cpv> = raw.cpv.iter().filter_map(|s| parse_cpv(s)).collect();

        // Join list fields
        let contract_types = join_list(&raw.tipo_contrato);
        let non_written_contract_justification_types =
            join_list(&raw.justif_n_reduc_escr_contrato);

        // ccp: not present in JSON, default to false
        let ccp = false;

        // Documents and invitees are not present in the JSON schema
        let documents = vec![];
        let invitees = vec![];

        // causes_deadline_change and causes_price_change are not in JSON
        let causes_deadline_change: Option<String> = None;
        let causes_price_change: Option<String> = None;

        Ok(Contract {
            id,
            contracting_procedure_type: raw.tipoprocedimento,
            publication_date,
            signing_date,
            ccp,
            object_brief_description: raw.objecto_contrato,
            initial_contractual_price: parse_euros(raw.preco_contratual),
            description,
            contracting,
            contracted,
            cpvs,
            regime,
            contract_status,
            non_written_contract_justification_types,
            contract_types,
            execution_deadline_days: raw.prazo_execucao.max(0) as usize,
            execution_places: raw.local_execucao,
            contract_fundamentation_type: raw.fundamentacao,
            contestants,
            invitees,
            documents,
            contracting_procedure_url,
            announcement_id,
            direct_award_fundamentation_type: raw.fundament_ajuste_direto,
            observations,
            end_of_contract_type,
            close_date,
            total_effective_price,
            causes_deadline_change,
            causes_price_change,
        })
    }
}

/// Collect all `.json` files from a directory (non-recursive).
pub fn collect_json_files(dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in
        std::fs::read_dir(dir).with_context(|| format!("Failed to read directory: {dir:?}"))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "json") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

/// Ingest all JSON files from a directory into the database and search index.
pub async fn ingest(
    contract_db: &ContractDatabase,
    search_db: &SearchDatabase,
    dir: &Path,
) -> anyhow::Result<(usize, usize, usize)> {
    let files = collect_json_files(dir)?;
    info!("Found {} JSON file(s) to ingest", files.len());

    let mut total_inserted = 0usize;
    let mut total_skipped = 0usize;
    let mut total_search_failures = 0usize;

    for file_path in &files {
        info!("Processing file: {}", file_path.display());
        let (inserted, skipped, search_failures) =
            ingest_file(contract_db, search_db, file_path).await?;
        total_inserted += inserted;
        total_skipped += skipped;
        total_search_failures += search_failures;
        info!(
            "Finished file {}: {} inserted, {} skipped (search failures: {})",
            file_path.display(),
            inserted,
            skipped,
            search_failures
        );
    }

    Ok((total_inserted, total_skipped, total_search_failures))
}

/// Ingest a single JSON file.
async fn ingest_file(
    contract_db: &ContractDatabase,
    search_db: &SearchDatabase,
    file_path: &Path,
) -> anyhow::Result<(usize, usize, usize)> {
    let raw = std::fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read: {}", file_path.display()))?;

    let raw_contracts: Vec<JsonContract> = serde_json::from_str(&raw)
        .with_context(|| format!("Failed to parse JSON: {}", file_path.display()))?;

    info!(
        "Loaded {} raw records from {}",
        raw_contracts.len(),
        file_path.display()
    );

    let mut inserted = 0usize;
    let mut skipped = 0usize;
    let mut search_failures = 0usize;

    for raw_contract in raw_contracts {
        let contract = match Contract::try_from(raw_contract) {
            Ok(c) => c,
            Err(e) => {
                warn!("Skipping record: {e}");
                skipped += 1;
                continue;
            }
        };

        // Insert into Postgres
        if let Err(e) = contract_db.insert_contract(&contract).await {
            error!("Failed to insert contract {} into DB: {e}", contract.id);
            skipped += 1;
            continue;
        }

        // Index in Meilisearch (non-fatal if it fails)
        if let Err(e) = search_db.save_contract(contract).await {
            error!("Failed to index contract in search: {e}");
            search_failures += 1;
        }

        inserted += 1;

        if inserted % 10_000 == 0 {
            info!("Progress: {} contracts inserted so far", inserted);
        }
    }

    Ok((inserted, skipped, search_failures))
}
