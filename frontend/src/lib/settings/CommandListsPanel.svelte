<script lang="ts">
  import { ask } from '@tauri-apps/plugin-dialog'
  import { errorText } from '$lib/bridge/errors'
  import { deleteUserCmdlist, openCmdlistsDir, writeUserCmdlist } from '$lib/bridge/commands'
  import { BUILTIN, builtinFileText } from '$lib/cmdlists'
  import { loadUserLists, userLists } from '$lib/cmdlists/userLists.svelte'
  import { CUSTOM_PLATFORM } from '$lib/cmdlists/custom'

  let status = $state('')
  let busy = $state(false)

  /** Every platform with a built-in list or a user file, built-in first. */
  const platforms = $derived([
    ...Object.keys(BUILTIN),
    ...Object.keys({ ...userLists.lists, ...userLists.errors }).filter(
      (p) => !(p in BUILTIN) && p !== CUSTOM_PLATFORM,
    ),
  ])

  function labelOf(platform: string): string {
    return userLists.lists[platform]?.label ?? BUILTIN[platform]?.label ?? platform
  }

  function countOf(platform: string): number {
    const v = userLists.lists[platform] ?? BUILTIN[platform]
    if (!v) return 0
    return v.lists.reduce((n, l) => n + l.sections.reduce((m, s) => m + s.items.length, 0), 0)
  }

  async function run(fn: () => Promise<void>) {
    busy = true
    status = ''
    try {
      await fn()
    } catch (e) {
      status = `Couldn't do it: ${errorText(e)}`
    } finally {
      busy = false
    }
  }

  const reload = () =>
    run(async () => {
      await loadUserLists()
      const bad = Object.keys(userLists.errors).length
      status = bad
        ? `Reloaded. ${bad} file${bad > 1 ? 's' : ''} with errors — the built-in list is used for ${bad > 1 ? 'them' : 'it'}.`
        : 'Reloaded.'
    })

  const makeCopy = (platform: string) =>
    run(async () => {
      const text = builtinFileText(platform)
      if (!text) return
      await writeUserCmdlist(platform, text)
      await loadUserLists()
      const dir = await openCmdlistsDir()
      status = `Copy created: ${platform}.json in ${dir}. Edit it and press Reload.`
    })

  const useBuiltin = (platform: string) =>
    run(async () => {
      const yes = await ask(
        `Delete your copy of ${labelOf(platform)} (${platform}.json) and use the built-in list again? Your changes in that file will be lost.`,
        { title: 'Use the built-in list', kind: 'warning' },
      )
      if (!yes) return
      await deleteUserCmdlist(platform)
      await loadUserLists()
      status = `${labelOf(platform)} uses the built-in list again.`
    })

  const openDir = () =>
    run(async () => {
      const dir = await openCmdlistsDir()
      status = `Folder: ${dir}`
    })
</script>

<div class="panel">
  <section>
    <h3>Command lists</h3>
    <p class="help">
      The lists under the terminal come from built-in JSON files, one per platform. To change one,
      make your own copy and edit it with any text editor: ZapTerm then uses your copy instead of
      the built-in list, and <b>updates never change your copy</b>. Remove it to go back to the
      built-in list (and get its updates).
    </p>
    <p class="help">
      Your own commands for every platform are in <b>★ Custom</b>, next to Recent in the bar (saved
      as <code>custom.json</code> in the same folder).
    </p>
    <div class="actions">
      <button type="button" class="btn" onclick={openDir} disabled={busy}>Open lists folder</button>
      <button type="button" class="btn" onclick={reload} disabled={busy}>Reload</button>
    </div>
    {#if status}<p class="status">{status}</p>{/if}
  </section>

  <section>
    <div class="rows">
      {#each platforms as p (p)}
        <div class="row">
          <div class="name">
            <span class="label">{labelOf(p)}</span>
            <code>{p}.json</code>
          </div>
          <div class="state">
            {#if userLists.errors[p]}
              <span class="badge err" title={userLists.errors[p]}>Error — using built-in</span>
            {:else if userLists.lists[p]}
              <span class="badge mine">Your copy</span>
            {:else}
              <span class="badge">Built-in</span>
            {/if}
            <span class="count">{countOf(p)} commands</span>
          </div>
          <div class="row-actions">
            {#if userLists.lists[p] || userLists.errors[p]}
              {#if p in BUILTIN}
                <button
                  type="button"
                  class="link danger"
                  onclick={() => useBuiltin(p)}
                  disabled={busy}>Use built-in</button
                >
              {/if}
            {:else}
              <button type="button" class="link" onclick={() => makeCopy(p)} disabled={busy}
                >Make my copy</button
              >
            {/if}
          </div>
        </div>
        {#if userLists.errors[p]}
          <p class="error-line">{p}.json: {userLists.errors[p]}</p>
        {/if}
      {/each}
    </div>
  </section>
</div>

<style>
  .panel {
    padding: 0.75rem 1rem 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 1.1rem;
    color: var(--zx-text);
    font-family: var(--zx-font-ui);
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 0.45rem;
  }
  h3 {
    font-size: 0.78rem;
    font-weight: 600;
    margin: 0 0 0.15rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .help {
    font-size: 0.74rem;
    color: var(--zx-text-muted);
    margin: 0;
    line-height: 1.5;
  }
  .help b {
    color: var(--zx-text);
  }
  .actions {
    display: flex;
    gap: 0.5rem;
  }
  .btn {
    background: var(--zx-surface-2);
    color: var(--zx-text);
    border: 1px solid var(--zx-border);
    border-radius: 0.3rem;
    padding: 0.32rem 0.75rem;
    font-size: 0.78rem;
    font-family: inherit;
    cursor: pointer;
  }
  .btn:hover:not(:disabled) {
    background: var(--zx-hover-bg);
  }
  .status {
    font-size: 0.74rem;
    color: var(--zx-text-muted);
    margin: 0;
    word-break: break-all;
  }
  .rows {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--zx-border);
    border-radius: 0.4rem;
    overflow: hidden;
  }
  .row {
    display: grid;
    grid-template-columns: 1fr auto auto;
    align-items: center;
    gap: 0.75rem;
    padding: 0.5rem 0.7rem;
    border-top: 1px solid var(--zx-border);
    font-size: 0.8rem;
  }
  .row:first-child {
    border-top: none;
  }
  .name {
    display: flex;
    flex-direction: column;
    gap: 0.1rem;
    min-width: 0;
  }
  .name code {
    font-family: var(--zx-font-mono);
    font-size: 0.7rem;
    color: var(--zx-text-dim);
  }
  .state {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .count {
    font-size: 0.7rem;
    color: var(--zx-text-dim);
    min-width: 6.5rem;
    text-align: right;
  }
  .badge {
    font-size: 0.68rem;
    padding: 0.1rem 0.45rem;
    border-radius: 999px;
    border: 1px solid var(--zx-border);
    color: var(--zx-text-muted);
    white-space: nowrap;
  }
  .badge.mine {
    color: var(--zx-accent);
    border-color: color-mix(in srgb, var(--zx-accent) 50%, transparent);
  }
  .badge.err {
    color: var(--zx-err);
    border-color: color-mix(in srgb, var(--zx-err) 50%, transparent);
  }
  .row-actions {
    min-width: 6.5rem;
    text-align: right;
  }
  .link {
    background: none;
    border: none;
    color: var(--zx-accent);
    font-size: 0.76rem;
    font-family: inherit;
    cursor: pointer;
    padding: 0;
  }
  .link.danger {
    color: var(--zx-err);
  }
  .link:disabled,
  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .error-line {
    margin: 0;
    padding: 0.35rem 0.7rem;
    font-size: 0.72rem;
    color: var(--zx-err);
    background: color-mix(in srgb, var(--zx-err) 8%, transparent);
    font-family: var(--zx-font-mono);
    word-break: break-word;
  }
</style>
