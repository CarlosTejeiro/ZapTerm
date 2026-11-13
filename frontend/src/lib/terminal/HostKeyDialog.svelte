<script lang="ts">
  import Icon from '$lib/icons/Icon.svelte'

  interface Props {
    host: string
    port: number
    fingerprint: string
    /** true = key changed (mismatch, danger); false = first connection (TOFU). */
    changed: boolean
    onTrust: () => void
    onCancel: () => void
  }

  let { host, port, fingerprint, changed, onTrust, onCancel }: Props = $props()

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onCancel()
  }
</script>

<div class="overlay" role="dialog" aria-modal="true" tabindex="-1" onkeydown={onKeydown}>
  <div class="dialog" class:danger={changed}>
    {#if changed}
      <h2 class="title danger-text"><Icon name="alert" size={15} /> Host key changed</h2>
      <p class="msg">
        The host key for <strong>{host}:{port}</strong> does not match the one recorded in
        <code>known_hosts</code>. This could indicate a man-in-the-middle attack, or the server was
        legitimately reinstalled.
      </p>
      <p class="msg">
        For safety, the connection is refused. If this change is expected (you reinstalled or
        factory-reset the device), you can replace the stored key with the new one below. Only do
        this if you trust the fingerprint shown.
      </p>
    {:else}
      <h2 class="title">First connection to {host}:{port}</h2>
      <p class="msg">
        This host is not yet in your <code>known_hosts</code>. Verify the fingerprint below through
        a trusted channel before continuing.
      </p>
    {/if}

    <div class="fp">
      <span>Fingerprint</span>
      <code>{fingerprint}</code>
    </div>

    <div class="actions">
      <button class="btn-cancel" onclick={onCancel}>Cancel</button>
      {#if changed}
        <button class="btn-overwrite" onclick={onTrust}>Replace key &amp; connect</button>
      {:else}
        <button class="btn-trust" onclick={onTrust}>Trust &amp; connect</button>
      {/if}
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    backdrop-filter: blur(6px);
    -webkit-backdrop-filter: blur(6px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 120;
  }

  .dialog {
    background: var(--zx-surface);
    border: 1px solid var(--zx-border);
    border-radius: 0.5rem;
    padding: 1.5rem;
    width: 30rem;
    max-width: 95vw;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    color: var(--zx-text);
    font-family: var(--zx-font-ui);
    box-shadow: var(--zx-shadow);
  }

  .dialog.danger {
    border-color: var(--zx-err);
  }

  .title {
    font-size: 0.95rem;
    font-weight: 600;
    color: var(--zx-text);
    margin: 0;
  }

  .danger-text {
    color: var(--zx-err);
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }

  .msg {
    font-size: 0.82rem;
    color: var(--zx-text-muted);
    line-height: 1.5;
    margin: 0;
  }

  .msg code,
  .fp code {
    color: var(--zx-text);
    background: var(--zx-surface-2);
    padding: 0 0.2rem;
    border-radius: 0.2rem;
  }

  .fp {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.75rem;
    color: var(--zx-text-muted);
  }

  .fp code {
    font-size: 0.8rem;
    word-break: break-all;
    padding: 0.4rem 0.5rem;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-top: 0.25rem;
  }

  .btn-cancel,
  .btn-trust {
    padding: 0.35rem 0.9rem;
    font-size: 0.8rem;
    border-radius: 0.25rem;
    border: none;
    cursor: pointer;
    font-family: inherit;
  }

  .btn-cancel {
    background: transparent;
    color: var(--zx-text-muted);
    border: 1px solid var(--zx-border);
  }

  .btn-cancel:hover {
    background: var(--zx-hover-bg);
    color: var(--zx-text);
  }

  .btn-trust {
    background: var(--zx-ok);
    color: var(--zx-on-accent);
  }

  .btn-trust:hover {
    background: color-mix(in srgb, var(--zx-ok) 85%, black);
  }

  /* A changed key is a destructive replace, so its action reads as danger,
     not the friendly green of a first-use trust. */
  .btn-overwrite {
    background: var(--zx-err);
    color: var(--zx-on-accent);
  }

  .btn-overwrite:hover {
    background: color-mix(in srgb, var(--zx-err) 85%, black);
  }
</style>
