import './app.css'
import App from './App.svelte'

const target = document.getElementById('app')
if (!target) {
  throw new Error('Could not find app element')
}

// Svelte 5 uses the mount function from 'svelte'
import { mount } from 'svelte'
const app = mount(App, { target })

export default app
