<script lang="ts">
  import { onMount } from 'svelte';
  import { commands } from './lib/bindings';
  import EmptyScreen from './lib/EmptyScreen.svelte';

  // M-01: the window asks the backend what it was started with. Opening the
  // given workspace comes with the workspace feature (M-05).
  onMount(async () => {
    try {
      const result = await commands.appStart();
      if (result.status === 'error') {
        console.error(`app_start failed: ${result.error.message}`);
      }
    } catch (error) {
      console.error('app_start failed', error);
    }
  });
</script>

<main>
  <EmptyScreen />
</main>

<style>
  main {
    height: 100%;
  }
</style>
