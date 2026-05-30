<script lang="ts">
  import { page } from '$app/stores';

  const links = [
    { href: '/', label: 'Home' },
    { href: '/decide', label: 'Decide' },
    { href: '/pre-call', label: 'Pre-Call' },
    { href: '/call', label: 'Call' },
    { href: '/post-call', label: 'Post-Call' },
    { href: '/audit', label: 'Audit' },
    { href: '/system', label: 'System' },
  ];
</script>

<nav>
  {#each links as link}
    <a href={link.href} class:active={$page.url.pathname === link.href}>{link.label}</a>
  {/each}
</nav>

<style>
  nav {
    display: flex;
    flex-direction: column;
    gap: var(--s-1);
    min-width: 140px;
  }

  a {
    padding: var(--s-2) var(--s-3);
    background: transparent;
    border: 1px solid var(--bg-4);
    border-radius: var(--radius);
    color: var(--fg-2);
    font-size: var(--text-sm);
    transition: all 0.15s;
    text-decoration: none;
  }

  a:hover {
    border-color: var(--cyan-dim);
    color: var(--fg-0);
  }

  a.active {
    background: var(--cyan-brand);
    color: var(--bg-0);
    border-color: var(--cyan-brand);
    font-weight: 600;
  }
</style>
