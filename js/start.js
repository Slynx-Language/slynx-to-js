function __start(){
  const c=main();
  Object.values(c).forEach(el=>{
    if(el instanceof HTMLElement) document.body.appendChild(el);
  });
}
__start();
