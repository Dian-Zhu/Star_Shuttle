<script>
  export let size = "100%";
  export let strokeWidth = 2;
  export let class_name = "";
  export { class_name as class };
</script>

<svg
  xmlns="http://www.w3.org/2000/svg"
  width={size}
  height={size}
  fill="none"
  viewBox="0 0 24 24"
  stroke="currentColor"
  stroke-width={strokeWidth}
  stroke-linecap="round"
  stroke-linejoin="round"
  class={class_name}
>
  <polyline points="9 18 15 12 9 6"></polyline>
</svg>
