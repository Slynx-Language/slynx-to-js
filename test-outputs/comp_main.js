main();
function Card(p0){
    let c1 = document.createElement("p");
     c1.textContent = "hello";
    return {c1,p0};
}
function main(){
    return Card("world");
}
function ApplyLStyle(p0,p1){
    p0.style.color = '#'+p1.f0.toString(16);
    return null;
}
function ApplyGayStyle(p0,p1){
ApplyLStyle(p0,p1)    p0.style.backgroundColor = '#'+p1.f0.toString(16);
    p0.style.color = '#'+p1.f1.toString(16);
    return null;
}
function __start(){
  const c=main();
  Object.values(c).forEach(el=>{
    if(el instanceof HTMLElement) document.body.appendChild(el);
  });
}
__start();
