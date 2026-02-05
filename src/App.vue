<script setup lang="ts">
import WaveformRenderer from './components/WaveformRenderer.vue';
import DiagnosticConsole from './components/DiagnosticConsole.vue';
import ArtifactTerminal from './components/ArtifactTerminal.vue';
import IntelligenceHub from './components/IntelligenceHub.vue';
</script>

<template>
  <div class="application-shell">
    <header class="masthead">
      <h1 class="masthead-title">SQL Execution Plan Analyzer</h1>
      <p class="masthead-subtitle">Interactive Query Performance Visualization System</p>
    </header>
    
    <div class="workspace-grid">
      <aside class="control-panel">
        <ArtifactTerminal />
      </aside>
      
      <main class="primary-viewport">
        <WaveformRenderer />
      </main>
      
      <aside class="analysis-panel">
        <DiagnosticConsole />
      </aside>
      
      <section class="intelligence-section">
        <IntelligenceHub />
      </section>
    </div>
  </div>
</template>

<style>
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

:root {
  font-family: 'Inter', 'Segoe UI', system-ui, -apple-system, sans-serif;
  font-size: 16px;
  line-height: 1.5;
  font-weight: 400;
  color: #f0f0f0;
  background-color: #0a0e1a;
  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}

body {
  margin: 0;
  padding: 0;
  width: 100vw;
  height: 100vh;
  overflow: hidden;
}

#app {
  width: 100%;
  height: 100%;
}
</style>

<style scoped>
.application-shell {
  width: 100%;
  height: 100vh;
  display: flex;
  flex-direction: column;
  background: linear-gradient(180deg, #0a0e1a 0%, #0d1421 100%);
  overflow: hidden;
}

.masthead {
  background: linear-gradient(135deg, #1a237e 0%, #283593 100%);
  padding: 20px 30px;
  border-bottom: 3px solid #3949ab;
  box-shadow: 0 4px 20px rgba(0, 0, 0, 0.5);
}

.masthead-title {
  font-size: 28px;
  font-weight: 900;
  color: #ffffff;
  margin-bottom: 6px;
  letter-spacing: -0.5px;
}

.masthead-subtitle {
  font-size: 14px;
  color: #b0c4de;
  font-weight: 500;
  letter-spacing: 0.5px;
}

.workspace-grid {
  flex: 1;
  display: grid;
  grid-template-columns: 320px 1fr 380px;
  grid-template-rows: 1fr 320px;
  gap: 16px;
  padding: 16px;
  overflow: hidden;
}

.control-panel {
  grid-column: 1;
  grid-row: 1 / 3;
  overflow-y: auto;
}

.primary-viewport {
  grid-column: 2;
  grid-row: 1;
  overflow: hidden;
}

.analysis-panel {
  grid-column: 3;
  grid-row: 1;
  overflow-y: auto;
}

.intelligence-section {
  grid-column: 2 / 4;
  grid-row: 2;
  overflow-y: auto;
}

::-webkit-scrollbar {
  width: 10px;
  height: 10px;
}

::-webkit-scrollbar-track {
  background: rgba(0, 0, 0, 0.3);
  border-radius: 5px;
}

::-webkit-scrollbar-thumb {
  background: rgba(100, 150, 255, 0.5);
  border-radius: 5px;
}

::-webkit-scrollbar-thumb:hover {
  background: rgba(100, 150, 255, 0.7);
}
</style>