<script lang="ts">
  export let title: string;
  export let ids: string[] = [];
</script>

<article class="player-bucket">
  <header>
    <strong>{title}</strong>
    <span>{ids.length}</span>
  </header>
  {#if ids.length}
    <ol>
      {#each ids.slice(0, 80) as id}
        <li>{id}</li>
      {/each}
    </ol>
    {#if ids.length > 80}<p>Showing first 80</p>{/if}
  {:else}
    <p>Empty</p>
  {/if}
</article>
