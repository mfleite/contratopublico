<script lang="ts">
  import { Upload, FileJson, CheckCircle, AlertCircle, XCircle, Loader, FileUp, Trash2 } from "@lucide/svelte";
  import { Button } from "$lib/components/ui/button";
  import { fade, fly } from "svelte/transition";

  interface IngestResult {
    inserted: number;
    skipped: number;
    search_failures: number;
    files_processed: number;
  }

  interface UploadFile {
    file: File;
    id: string;
  }

  type UploadStatus =
    | { state: "idle" }
    | { state: "uploading" }
    | { state: "success"; result: IngestResult }
    | { state: "error"; message: string };

  let files: UploadFile[] = $state([]);
  let status: UploadStatus = $state({ state: "idle" });
  let dragOver = $state(false);
  let fileInput: HTMLInputElement | undefined = $state();

  function handleDragOver(e: DragEvent) {
    e.preventDefault();
    dragOver = true;
  }

  function handleDragLeave(e: DragEvent) {
    e.preventDefault();
    dragOver = false;
  }

  function handleDrop(e: DragEvent) {
    e.preventDefault();
    dragOver = false;
    if (e.dataTransfer?.files) {
      addFiles(Array.from(e.dataTransfer.files));
    }
  }

  function addFiles(newFiles: File[]) {
    const jsonFiles = newFiles.filter((f) => f.name.endsWith(".json"));
    for (const file of jsonFiles) {
      files.push({ file, id: crypto.randomUUID() });
    }
  }

  function handleFileInputChange(e: Event) {
    const input = e.target as HTMLInputElement;
    if (input.files) {
      addFiles(Array.from(input.files));
    }
  }

  function removeFile(id: string) {
    files = files.filter((f) => f.id !== id);
  }

  async function upload() {
    if (files.length === 0) return;

    status = { state: "uploading" };

    const formData = new FormData();
    for (const { file } of files) {
      formData.append("files", file);
    }

    try {
      const response = await fetch("/api/admin/ingest", {
        method: "POST",
        body: formData,
      });

      if (!response.ok) {
        const errorData = await response.json().catch(() => ({ message: "Erro desconhecido" }));
        status = { state: "error", message: errorData.message || `Erro ${response.status}` };
        return;
      }

      const result: IngestResult = await response.json();
      status = { state: "success", result };
      files = [];
    } catch (e) {
      status = {
        state: "error",
        message: e instanceof Error ? e.message : "Falha na ligação ao servidor",
      };
    }
  }

  function reset() {
    status = { state: "idle" };
    files = [];
  }

  function formatBytes(bytes: number): string {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
  }

  function formatNumber(n: number): string {
    return n.toLocaleString("pt-PT");
  }

  const totalFilesSize = $derived(
    files.reduce((acc, f) => acc + f.file.size, 0)
  );

  const isLoading = $derived(status.state === "uploading");
</script>

<svelte:head>
  <title>Admin — Contrato Público</title>
</svelte:head>

<div class="mx-auto max-w-2xl space-y-8">
  <div class="space-y-1">
    <h1 class="text-3xl font-semibold tracking-tight">Painel de Administração</h1>
    <p class="text-muted-foreground">
      Importe ficheiros JSON com contratos públicos para a base de dados.
    </p>
  </div>

  {#if status.state === "idle" || status.state === "error"}
    <!-- Upload Zone -->
    <div
      class="border-border {dragOver ? 'border-primary bg-primary/5' : ''} relative rounded-xl border-2 border-dashed p-10 transition-colors"
      ondragover={handleDragOver}
      ondragleave={handleDragLeave}
      ondrop={handleDrop}
      role="button"
      tabindex="0"
    >
      <div class="flex flex-col items-center gap-4 text-center">
        <div
          class="bg-muted flex h-16 w-16 items-center justify-center rounded-full"
        >
          {#if dragOver}
            <FileUp class="text-primary h-8 w-8" />
          {:else}
            <Upload class="text-muted-foreground h-8 w-8" />
          {/if}
        </div>
        <div class="space-y-1">
          <p class="text-lg font-medium">
            {dragOver ? "Largue os ficheiros aqui" : "Arraste ficheiros JSON para aqui"}
          </p>
          <p class="text-muted-foreground text-sm">
            ou clique para selecionar ficheiros
          </p>
        </div>
        <input
          type="file"
          multiple
          accept=".json"
          class="absolute inset-0 cursor-pointer opacity-0"
          onchange={handleFileInputChange}
          bind:this={fileInput}
        />
      </div>
    </div>

    <!-- Error Banner -->
    {#if status.state === "error"}
      <div
        class="bg-destructive/10 text-destructive border-destructive/20 flex items-start gap-3 rounded-lg border p-4"
        transition:fly={{ y: -10, duration: 200 }}
      >
        <AlertCircle class="mt-0.5 h-5 w-5 shrink-0" />
        <div class="flex-1">
          <p class="font-medium">Erro na importação</p>
          <p class="text-sm opacity-80">{status.message}</p>
        </div>
        <button
          class="hover:bg-destructive/20 rounded-md p-1 transition-colors"
          onclick={reset}
          aria-label="Fechar"
        >
          <XCircle class="h-4 w-4" />
        </button>
      </div>
    {/if}

    <!-- Selected Files List -->
    {#if files.length > 0}
      <div class="space-y-3" transition:slide={{ duration: 200 }}>
        <div class="flex items-center justify-between">
          <h2 class="text-sm font-medium">
            {files.length} ficheiro{files.length !== 1 ? "s" : ""} selecionado{files.length !== 1 ? "s" : ""}
            <span class="text-muted-foreground ml-1 font-normal">
              ({formatBytes(totalFilesSize)})
            </span>
          </h2>
        </div>

        <ul class="border-border divide-border divide-y rounded-lg border">
          {#each files as { file, id } (id)}
            <li
              class="flex items-center justify-between px-4 py-3"
              transition:slide={{ duration: 150 }}
            >
              <div class="flex items-center gap-3 min-w-0">
                <FileJson class="text-muted-foreground h-5 w-5 shrink-0" />
                <span class="truncate text-sm">{file.name}</span>
                <span class="text-muted-foreground shrink-0 text-xs tabular-nums">
                  {formatBytes(file.size)}
                </span>
              </div>
              <button
                class="text-muted-foreground hover:text-destructive ml-3 rounded p-1 transition-colors"
                onclick={() => removeFile(id)}
                disabled={isLoading}
                aria-label="Remover {file.name}"
              >
                <Trash2 class="h-4 w-4" />
              </button>
            </li>
          {/each}
        </ul>

        <div class="flex gap-3">
          <Button
            class="flex-1"
            disabled={isLoading}
            onclick={reset}
            variant="outline"
          >
            Limpar
          </Button>
          <Button class="flex-1" disabled={isLoading} onclick={upload}>
            <Upload class="mr-2 h-4 w-4" />
            Importar {files.length} ficheiro{files.length !== 1 ? "s" : ""}
          </Button>
        </div>
      </div>
    {/if}
  {:else if status.state === "uploading"}
    <!-- Uploading State -->
    <div class="border-border bg-card rounded-xl border p-10" transition:fade={{ duration: 200 }}>
      <div class="flex flex-col items-center gap-6 text-center">
        <Loader class="text-primary h-10 w-10 animate-spin" />
        <div class="space-y-1">
          <p class="text-lg font-medium">A importar contratos...</p>
          <p class="text-muted-foreground text-sm">
            Isto pode demorar vários minutos dependendo do tamanho dos ficheiros.
          </p>
        </div>
      </div>
    </div>
  {:else if status.state === "success"}
    <!-- Success State -->
    <div class="space-y-6" transition:fade={{ duration: 200 }}>
      <!-- Success Card -->
      <div
        class="bg-card border-border rounded-xl border p-8"
        in:fly={{ y: 20, duration: 300 }}
      >
        <div class="flex flex-col items-center gap-4 text-center">
          <div class="bg-emerald-500/10 flex h-16 w-16 items-center justify-center rounded-full">
            <CheckCircle class="text-emerald-500 h-8 w-8" />
          </div>
          <div class="space-y-1">
            <p class="text-xl font-semibold">Importação concluída</p>
            <p class="text-muted-foreground text-sm">
              {status.result.files_processed} ficheiro{status.result.files_processed !== 1 ? "s" : ""} processado{status.result.files_processed !== 1 ? "s" : ""}
            </p>
          </div>
        </div>
      </div>

      <!-- Stats Grid -->
      <div class="grid grid-cols-3 gap-4">
        <div
          class="bg-card border-border rounded-lg border p-5 text-center"
          in:fly={{ y: 20, duration: 300, delay: 100 }}
        >
          <p class="text-3xl font-bold tabular-nums">{formatNumber(status.result.inserted)}</p>
          <p class="text-muted-foreground mt-1 text-sm">Contratos inseridos</p>
        </div>

        <div
          class="bg-card border-border rounded-lg border p-5 text-center"
          in:fly={{ y: 20, duration: 300, delay: 150 }}
        >
          <p class="text-3xl font-bold tabular-nums">{formatNumber(status.result.skipped)}</p>
          <p class="text-muted-foreground mt-1 text-sm">Ignorados</p>
        </div>

        <div
          class="bg-card border-border rounded-lg border p-5 text-center"
          in:fly={{ y: 20, duration: 300, delay: 200 }}
        >
          <p class="text-3xl font-bold tabular-nums">{formatNumber(status.result.search_failures)}</p>
          <p class="text-muted-foreground mt-1 text-sm">Falhas de pesquisa</p>
        </div>
      </div>

      <!-- Warning for search failures -->
      {#if status.result.search_failures > 0}
        <div
          class="bg-amber-500/10 text-amber-700 dark:text-amber-400 border-amber-500/20 flex items-start gap-3 rounded-lg border p-4"
          in:fly={{ y: 10, duration: 200, delay: 300 }}
        >
          <AlertCircle class="mt-0.5 h-5 w-5 shrink-0" />
          <div>
            <p class="font-medium">Falhas na indexação de pesquisa</p>
            <p class="text-sm opacity-80">
              {status.result.search_failures} contrato{status.result.search_failures !== 1 ? "s" : ""} não foram indexados no Meilisearch.
              Pode reconstruir o índice de pesquisa posteriormente.
            </p>
          </div>
        </div>
      {/if}

      <div class="flex justify-center">
        <Button onclick={reset} variant="outline" size="lg">
          Importar mais ficheiros
        </Button>
      </div>
    </div>
  {/if}
</div>
