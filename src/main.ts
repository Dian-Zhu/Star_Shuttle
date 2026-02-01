import '@xterm/xterm/css/xterm.css';  // 先加载 xterm 默认样式
import './app.css';                   // 再加载我们的覆盖样式
import App from './App.svelte'

const target = document.getElementById('app')
if (!target) {
  throw new Error('Could not find app element')
}

// Svelte 5 uses the mount function from 'svelte'
import { mount } from 'svelte'
const app = mount(App, { target })

export default app
