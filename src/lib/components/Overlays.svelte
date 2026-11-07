<script lang="ts">
  import { open as pickFolder } from '@tauri-apps/plugin-dialog';
  import FolderSimple from 'phosphor-svelte/lib/FolderSimple';
  import Plus from 'phosphor-svelte/lib/Plus';
  import WarningCircle from 'phosphor-svelte/lib/WarningCircle';
  import { app } from '../app.svelte';
  import { answer, dialog } from '../confirm.svelte';
  import { answerDestination, destinationDialog, isAbsolutePath } from '../destination.svelte';
  import { t, tm } from '../i18n.svelte';
  import { answerProviders, providerDialog, toggleAllProviders, toggleProvider } from '../providers.svelte';
  import { providerLabel } from '../types';

  let cancelButton: HTMLButtonElement | undefined = $state();
  let confirmButton: HTMLButtonElement | undefined = $state();
  $effect(() => {
    if (dialog.open) cancelButton?.focus();
  });
  // Land on the confirm button so a quick Enter accepts the Settings folder.
  $effect(() => {
    if (destinationDialog.open) confirmButton?.focus();
  });
  let addButton: HTMLButtonElement | undefined = $state();
  $effect(() => {
    if (providerDialog.open) addButton?.focus();
  });

  const accountLine = (provider: (typeof providerDialog.options)[number]) => {
    const user = app.accounts.find((a) => a.provider === provider)?.user;
    return user ? `${user.username} · ${user.kind}` : t('Chưa kiểm tra token mới');
  };

  const validDirectory = $derived(isAbsolutePath(destinationDialog.directory));

  async function browse() {
    const picked = await pickFolder({ directory: true, multiple: false, title: t('Chọn nơi lưu toàn bộ torrent'), defaultPath: destinationDialog.directory.trim() || undefined });
    if (typeof picked === 'string') destinationDialog.directory = picked;
  }

  function keydown(event: KeyboardEvent) {
    if (event.key !== 'Escape') return;
    if (destinationDialog.open) answerDestination(false);
    else if (providerDialog.open) answerProviders(false);
    else if (dialog.open) answer(false);
  }
</script>

<svelte:window onkeydown={keydown} />

{#if dialog.open && dialog.request}
  <div class="scrim" role="presentation">
    <div class="dialog card" role="alertdialog" aria-modal="true" aria-labelledby="dialog-title">
      <h2 id="dialog-title" class="section">{tm(dialog.request.title)}</h2>
      <p>{tm(dialog.request.body)}</p>
      <div class="actions">
        <button class="btn outline" bind:this={cancelButton} onclick={() => answer(false)}>{t('Hủy')}</button>
        <button class="btn primary" class:danger-fill={dialog.request.danger} onclick={() => answer(true)}>{tm(dialog.request.confirm)}</button>
      </div>
    </div>
  </div>
{/if}

{#if destinationDialog.open}
  <div class="scrim" role="presentation">
    <div class="dialog card" role="dialog" aria-modal="true" aria-labelledby="destination-title">
    <form class="form" onsubmit={(event) => { event.preventDefault(); answerDestination(true); }}>
      <h2 id="destination-title" class="section">{t('Thêm vào hàng đợi')}</h2>
      <p>{t('Lưu {0} file vào thư mục:', destinationDialog.files)}</p>
      <div class="pair">
        <input type="text" bind:value={destinationDialog.directory} aria-label={t('Thư mục lưu')} aria-invalid={!validDirectory} spellcheck="false" />
        <button type="button" class="btn outline icon" onclick={browse} title={t('Chọn thư mục')} aria-label={t('Chọn thư mục')}><span class="ic amber"><FolderSimple size={17} weight="duotone" /></span></button>
      </div>
      {#if !validDirectory}<p class="invalid">{t('Chọn một đường dẫn lưu đầy đủ.')}</p>{/if}
      <div class="actions">
        <button type="button" class="btn outline" onclick={() => answerDestination(false)}>{t('Hủy')}</button>
        <button type="submit" class="btn primary" bind:this={confirmButton} disabled={!validDirectory}><Plus size={16} weight="bold" />{t('Thêm vào hàng đợi')}</button>
      </div>
    </form>
    </div>
  </div>
{/if}

{#if providerDialog.open}
  <div class="scrim" role="presentation">
    <div class="dialog card" role="dialog" aria-modal="true" aria-labelledby="providers-title">
    <form class="form" onsubmit={(event) => { event.preventDefault(); answerProviders(true); }}>
      <h2 id="providers-title" class="section">{t('Thêm vào thư viện')}</h2>
      <p>{t('Thêm {0} torrent vào dịch vụ nào?', providerDialog.count)}</p>
      <div class="choices" role="group" aria-label={t('Dịch vụ tải')}>
        <label class="choice all">
          <input type="checkbox" checked={providerDialog.chosen.length === providerDialog.options.length} indeterminate={providerDialog.chosen.length > 0 && providerDialog.chosen.length < providerDialog.options.length} onchange={toggleAllProviders} />
          <b>{t('Tất cả dịch vụ')}</b>
        </label>
        {#each providerDialog.options as provider (provider)}
          <label class="choice">
            <input type="checkbox" checked={providerDialog.chosen.includes(provider)} onchange={() => toggleProvider(provider)} />
            <span class="who"><b>{providerLabel(provider)}</b><small>{accountLine(provider)}</small></span>
          </label>
        {/each}
      </div>
      {#if providerDialog.chosen.length === 0}<p class="invalid">{t('Chọn ít nhất một dịch vụ.')}</p>{/if}
      <label class="choice plain"><input type="checkbox" bind:checked={providerDialog.remember} />{t('Nhớ lựa chọn này, lần sau không hỏi lại')}</label>
      <div class="actions">
        <button type="button" class="btn outline" onclick={() => answerProviders(false)}>{t('Hủy')}</button>
        <button type="submit" class="btn primary" bind:this={addButton} disabled={providerDialog.chosen.length === 0}><Plus size={16} weight="bold" />{t('Thêm vào thư viện')}</button>
      </div>
    </form>
    </div>
  </div>
{/if}

<div class="toasts" aria-live="polite">
  {#each app.toasts as toast (toast.id)}
    <div class="toast card {toast.tone}">{#if toast.tone === 'warning'}<WarningCircle size={18} weight="duotone" aria-hidden="true" /><span>{tm(toast.message)}</span>{:else}{tm(toast.message)}{/if}</div>
  {/each}
</div>

<style>
  .scrim { position: fixed; inset: 0; background: rgb(10 10 11 / 0.36); backdrop-filter: blur(4px); display: grid; place-content: center; z-index: 50; }
  .dialog { width: min(480px, 90vw); padding: 24px 26px; display: flex; flex-direction: column; gap: 12px; background: var(--glass-strong); box-shadow: var(--float); backdrop-filter: blur(20px); border-radius: 24px; }
  .dialog p { color: var(--muted); line-height: 1.55; user-select: text; }
  .form { display: flex; flex-direction: column; gap: 12px; }
  .pair { display: flex; gap: 8px; }
  .invalid { color: var(--danger); font-size: 12px; }
  .choices { display: flex; flex-direction: column; gap: 2px; padding: 4px; border-radius: 16px; background: var(--subtle); }
  .choice { display: flex; align-items: center; gap: 12px; padding: 9px 12px; border-radius: 12px; cursor: pointer; transition: background 0.15s; }
  .choice:hover { background: var(--hover); }
  .choice.all { color: var(--ink); }
  .choice.plain { padding: 4px 2px; color: var(--muted); }
  .choice.plain:hover { background: none; color: var(--ink); }
  .who { display: flex; flex-direction: column; min-width: 0; line-height: 1.3; }
  .who small { color: var(--muted); font-size: 11px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .actions { display: flex; justify-content: flex-end; gap: 10px; margin-top: 8px; }
  .danger-fill, .danger-fill:hover:not(:disabled) { background: var(--icon-red); color: #fff; }
  .toasts { position: fixed; right: 20px; bottom: 48px; display: flex; flex-direction: column; gap: 8px; z-index: 70; max-width: 420px; }
  .toast { padding: 11px 16px; border-radius: 16px; background: var(--glass-strong); box-shadow: var(--float); backdrop-filter: blur(20px); }
  .toast.error { color: var(--danger); }
  .toast.success { color: var(--success); }
  .toast.warning { display: flex; align-items: center; gap: 8px; color: var(--partial); }
</style>
