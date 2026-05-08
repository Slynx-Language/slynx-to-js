main();
function Card(p0){
    let c1 = document.createElement("p");
     c1.textContent = "hello";
    return {c1,p0};
}
function main(){
    return Card("world");
}
function __start(){
  const c=main();
  Object.values(c).forEach(el=>{
    if(el instanceof HTMLElement) document.body.appendChild(el);
  });
}
__start();
