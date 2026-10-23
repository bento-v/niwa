<script lang="ts">
  import Plant from '../lib/Plant.svelte';
  import { SPECIES_IDS, STAGE_NAMES } from '../lib/plants';
  import Scene from '../lib/Scene.svelte';
</script>

<!-- Every plant drawing on one page, for reviewing the art. Development only: open /#art. -->
<div class="sheet">
  <h1 class="display">Plant art</h1>
  <table>
    <thead>
      <tr>
        <th></th>
        {#each STAGE_NAMES as name, i}
          <th>{i + 1} · {name}</th>
        {/each}
      </tr>
    </thead>
    <tbody>
      {#each SPECIES_IDS as species}
        <tr>
          <th>{species}</th>
          {#each STAGE_NAMES as _, i}
            <td><Plant {species} stage={i + 1} detailed width={120} /></td>
          {/each}
        </tr>
      {/each}
      <tr>
        <th>withered</th>
        <td><Plant species="pine" withered detailed width={120} /></td>
      </tr>
    </tbody>
  </table>

  <h1 class="display">At tile and picker size</h1>
  <div class="row">
    {#each SPECIES_IDS as species}
      <Plant {species} width={76} detailed />
    {/each}
    <Plant species="pine" withered width={76} detailed />
  </div>
  <div class="row">
    {#each SPECIES_IDS as species}
      {#each STAGE_NAMES as _, i}
        <Plant {species} stage={i + 1} width={52} />
      {/each}
    {/each}
  </div>

  <h1 class="display">In the scene</h1>
  <div class="scenes">
    {#each SPECIES_IDS as species}
      <div><Scene {species} tall label={species} /></div>
    {/each}
    <div><Scene species="pine" withered tall label="withered" /></div>
  </div>
</div>

<style>
  .sheet {
    width: 100%;
    max-width: 1180px;
    padding: 24px;
    box-sizing: border-box;
    background: var(--card);
    border-radius: 22px;
    user-select: text;
  }
  h1 {
    font-size: 24px;
    margin: 18px 0 10px;
  }
  h1:first-child {
    margin-top: 0;
  }
  table {
    border-collapse: collapse;
  }
  th {
    font-size: 12px;
    font-weight: 600;
    color: var(--muted);
    text-transform: capitalize;
    padding: 4px 10px;
    text-align: left;
  }
  td {
    padding: 4px;
    background: var(--tile);
    border: 4px solid var(--card);
    border-radius: 14px;
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 10px;
  }
  .scenes {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
  }
  .scenes div {
    width: 380px;
  }
</style>
