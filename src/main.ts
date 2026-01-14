import './app.css'
import App from './App.svelte'

const target = document.getElementById('app')
if (!target) {
  throw new Error('Could not find app element')
}

// For Svelte 5, we need to use the compatibility mode by updating svelte.config.js
// This will allow us to use the Svelte 4 API in Svelte 5
const app = new App({
  target,
})

export default app
