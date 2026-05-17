main();
function Pedrinho(){
    let c0 = document.createElement('div');
    PedrinhoInit(c0);
    return {c1};
}
function Jorgin(){
    let c0 = document.createElement('p');
    let c1 = Pedrinho();
    JorginInit(c0);
    return {c1,c2};
}
function PedrinhoInit(p0){
    let v1 = document.createElement('p');
    v1.textContent = "Pedrinho leitazedo";
    p0.appendChild(v1);
    return null;
}
function JorginInit(p0){
    p0.textContent = "Jorginho neguinho";
    return null;
}
function main(){
    let v1 = Jorgin();
    return v1;
}
function __start(){
  const c=main();
  Object.values(c).forEach(el=>{
    if(el instanceof HTMLElement) document.body.appendChild(el);
  });
}
__start();
