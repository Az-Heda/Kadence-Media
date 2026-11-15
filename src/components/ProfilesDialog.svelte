<script lang="ts">
  import { api, errMsg, type ProfileInfo } from '../lib/api';
  import { app, switchProfile } from '../lib/store.svelte';
  import { t, th } from '../lib/i18n.svelte';
  import ConfirmDialog from './ConfirmDialog.svelte';
  import UserRound from '@lucide/svelte/icons/user-round';
  import Lock from '@lucide/svelte/icons/lock';
  import KeyRound from '@lucide/svelte/icons/key-round';
  import Trash2 from '@lucide/svelte/icons/trash-2';
  import Check from '@lucide/svelte/icons/check';
  import Plus from '@lucide/svelte/icons/plus';

  let { onclose }: { onclose: () => void } = $props();

  // What the dialog is showing. 'pin' asks for the PIN needed by a switch or a delete.
  type View =
    | { kind: 'list' }
    | { kind: 'create' }
    | { kind: 'pin'; action: 'switch' | 'delete'; profile: ProfileInfo }
    | { kind: 'edit-pin'; profile: ProfileInfo };
  let view = $state<View>({ kind: 'list' });
  let confirmDelete = $state<ProfileInfo | null>(null);
  let error = $state('');
  let busy = $state(false);

  let name = $state('');
  let pin = $state('');
  let newPin = $state('');

  function go(v: View) {
    view = v;
    error = '';
    name = pin = newPin = '';
  }

  /** Run a request; failures are shown inside the dialog (toasts would sit behind it). */
  async function run(fn: () => Promise<unknown>): Promise<boolean> {
    busy = true;
    error = '';
    try {
      await fn();
      return true;
    } catch (e) {
      error = errMsg(e);
      return false;
    } finally {
      busy = false;
    }
  }

  async function pick(p: ProfileInfo) {
    if (p.id === app.profiles.current) return onclose();
    if (p.has_pin) return go({ kind: 'pin', action: 'switch', profile: p });
    await doSwitch(p.id, null);
  }
  async function doSwitch(id: number, pinValue: string | null) {
    busy = true;
    error = '';
    const err = await switchProfile(id, pinValue);
    busy = false;
    if (err) error = err; else onclose();
  }

  async function submitPin() {
    if (view.kind !== 'pin') return;
    const { action, profile } = view;
    if (action === 'switch') return doSwitch(profile.id, pin);
    if (await run(async () => (app.profiles = await api.deleteProfile(profile.id, pin)))) go({ kind: 'list' });
  }

  async function create() {
    if (await run(async () => (app.profiles = await api.createProfile(name, pin || null)))) go({ kind: 'list' });
  }

  async function savePin() {
    if (view.kind !== 'edit-pin') return;
    const p = view.profile;
    if (await run(async () => (app.profiles = await api.setProfilePin(p.id, p.has_pin ? pin : null, newPin || null)))) go({ kind: 'list' });
  }

  function askDelete() {
    const p = confirmDelete;
    confirmDelete = null;
    if (!p) return;
    if (p.has_pin) go({ kind: 'pin', action: 'delete', profile: p });
    else run(async () => (app.profiles = await api.deleteProfile(p.id, null)));
  }

  const focusOnMount = (el: HTMLInputElement) => el.focus();
  // Digits only, at most 8.
  const digits = (v: string) => v.replace(/\D/g, '').slice(0, 8);
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && !confirmDelete && (view.kind === 'list' ? onclose() : go({ kind: 'list' }))} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={onclose}>
  <div class="dialog card" role="dialog" aria-modal="true" tabindex="-1" onclick={(e) => e.stopPropagation()}>
    {#if view.kind === 'list'}
      <h1>{t('profiles.title')}</h1>
      <div class="muted intro">{t('profiles.intro')}</div>
      <div class="list">
        {#each app.profiles.profiles as p (p.id)}
          <div class="row item" class:current={p.id === app.profiles.current}>
            <button class="ghost grow pick" disabled={busy} onclick={() => pick(p)}>
              <UserRound size={16} />
              <span class="pname">{p.name}</span>
              {#if p.has_pin}<span class="muted" title={t('profiles.protected')}><Lock size={13} /></span>{/if}
              {#if p.id === app.profiles.current}<span class="cur"><Check size={14} /> {t('profiles.current')}</span>{/if}
            </button>
            {#if !p.is_default}
              <button class="ghost" title={t(p.has_pin ? 'profiles.change_pin' : 'profiles.set_pin')} onclick={() => go({ kind: 'edit-pin', profile: p })}><KeyRound size={15} /></button>
            {/if}
            {#if !p.is_default && p.id !== app.profiles.current}
              <button class="ghost danger" title={t('common.delete')} onclick={() => (confirmDelete = p)}><Trash2 size={15} /></button>
            {/if}
          </div>
        {/each}
      </div>
      {#if error}<div class="error">{error}</div>{/if}
      <div class="row buttons">
        <button onclick={() => go({ kind: 'create' })}><Plus size={15} /> {t('profiles.new')}</button>
        <span class="grow"></span>
        <button onclick={onclose}>{t('common.close')}</button>
      </div>
    {:else if view.kind === 'create'}
      <form onsubmit={(e) => { e.preventDefault(); create(); }}>
        <h1>{t('profiles.new')}</h1>
        <label class="field">{t('profiles.name')}
          <input type="text" maxlength="40" bind:value={name} use:focusOnMount placeholder={t('profiles.name_placeholder')} />
        </label>
        <label class="field">{t('profiles.pin_optional')}
          <input type="password" inputmode="numeric" autocomplete="off" value={pin} oninput={(e) => (pin = e.currentTarget.value = digits(e.currentTarget.value))} />
        </label>
        <div class="muted hint">{t('profiles.pin_hint')}</div>
        {#if error}<div class="error">{error}</div>{/if}
        <div class="row buttons">
          <span class="grow"></span>
          <button type="button" onclick={() => go({ kind: 'list' })}>{t('common.cancel')}</button>
          <button class="primary" type="submit" disabled={busy || !name.trim()}>{t('common.create')}</button>
        </div>
      </form>
    {:else if view.kind === 'pin'}
      <form onsubmit={(e) => { e.preventDefault(); submitPin(); }}>
        <h1>{t('profiles.enter_pin')}</h1>
        <div class="muted intro">{@html th(view.action === 'switch' ? 'profiles.pin_for_switch' : 'profiles.pin_for_delete', { name: view.profile.name })}</div>
        <input type="password" inputmode="numeric" autocomplete="off" class="full" use:focusOnMount
          value={pin} oninput={(e) => (pin = e.currentTarget.value = digits(e.currentTarget.value))} />
        {#if error}<div class="error">{error}</div>{/if}
        <div class="row buttons">
          <span class="grow"></span>
          <button type="button" onclick={() => go({ kind: 'list' })}>{t('common.cancel')}</button>
          <button class="primary" type="submit" disabled={busy || !pin}>{t('common.confirm')}</button>
        </div>
      </form>
    {:else}
      <form onsubmit={(e) => { e.preventDefault(); savePin(); }}>
        <h1>{t(view.profile.has_pin ? 'profiles.change_pin' : 'profiles.set_pin')}</h1>
        <div class="muted intro">{@html th('profiles.pin_of', { name: view.profile.name })}</div>
        {#if view.profile.has_pin}
          <label class="field">{t('profiles.current_pin')}
            <input type="password" inputmode="numeric" autocomplete="off" use:focusOnMount
              value={pin} oninput={(e) => (pin = e.currentTarget.value = digits(e.currentTarget.value))} />
          </label>
        {/if}
        <label class="field">{t('profiles.new_pin')}
          <input type="password" inputmode="numeric" autocomplete="off"
            value={newPin} oninput={(e) => (newPin = e.currentTarget.value = digits(e.currentTarget.value))} />
        </label>
        <div class="muted hint">{t(view.profile.has_pin ? 'profiles.remove_hint' : 'profiles.pin_hint')}</div>
        {#if error}<div class="error">{error}</div>{/if}
        <div class="row buttons">
          <span class="grow"></span>
          <button type="button" onclick={() => go({ kind: 'list' })}>{t('common.cancel')}</button>
          <button class="primary" type="submit" disabled={busy || (!view.profile.has_pin && !newPin) || (view.profile.has_pin && !pin)}>{t('common.save')}</button>
        </div>
      </form>
    {/if}
  </div>
</div>

{#if confirmDelete}
  <ConfirmDialog title={t('profiles.delete_title')} onconfirm={askDelete} oncancel={() => (confirmDelete = null)}>
    <p>{@html th('profiles.delete_body', { name: confirmDelete.name })}</p>
  </ConfirmDialog>
{/if}

<style>
  .backdrop { position: fixed; inset: 0; background: #000a; display: grid; place-items: center; z-index: 60; }
  .dialog { width: min(460px, 92vw); max-height: 90vh; overflow-y: auto; }
  .intro { margin-bottom: 14px; line-height: 1.45; }
  .list { display: flex; flex-direction: column; gap: 6px; }
  .item { border: 1px solid var(--border); border-radius: 10px; padding: 2px 4px; }
  .item.current { border-color: var(--primary); }
  .pick { display: flex; align-items: center; gap: 8px; justify-content: flex-start; text-align: left; }
  .pname { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .cur { margin-left: auto; color: var(--primary); display: inline-flex; align-items: center; gap: 4px; font-size: 12px; }
  .field { display: flex; flex-direction: column; gap: 5px; margin-top: 12px; }
  .full { width: 100%; }
  .hint { font-size: 12px; margin-top: 8px; }
  .error { color: var(--danger); margin-top: 12px; font-size: 13px; }
  .buttons { margin-top: 18px; }
</style>
