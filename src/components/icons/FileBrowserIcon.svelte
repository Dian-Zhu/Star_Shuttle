<script>
  export let size = "20";
  export let color = "currentColor";
  export { className as class };
  let className = "";
</script>

<svg
  xmlns="http://www.w3.org/2000/svg"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke={color}
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
  class={className}
>
  <path d="M4 4h5l2 2h9a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2z"></path>
  <line x1="6" y1="12" x2="6" y2="12"></line>
  <line x1="9" y1="12" x2="18" y2="12"></line>
  <line x1="6" y1="15.5" x2="6" y2="15.5"></line>
  <line x1="9" y1="15.5" x2="18" y2="15.5"></line>
</svg>
