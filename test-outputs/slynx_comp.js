function Pedrinho(){
    let c0 = document.createElement('div');
    PedrinhoInit(c0);
    return {c0};
}
function Jorgin(){
    let c0 = document.createElement('p');
    let c1 = Pedrinho();
    JorginInit(c0);
    Fg(c0,__init_Fg(16711935));
    return {c0,c1};
}
function __init_Bg(p0){
    return {};
}
function Bg(p0,p1){
    return null;
}
function __init_Fg(p0){
    return {};
}
function Fg(p0,p1){
    return null;
}
function PedrinhoInit(p0){
    let v0 = document.createElement('p');
    v0.textContent = "Pedrinho leitazedo";
    p0.appendChild(v0);
    return null;
}
function JorginInit(p0){
    p0.textContent = "Jorginho neguinho";
    return null;
}
function main(){
    let v0 = Jorgin();
    return v0;
}

function extractAndAppend(obj) {
  Object.values(obj).forEach(el => {
    if (el instanceof HTMLElement) {
      document.body.appendChild(el);
    } else if (typeof el === 'object' && el !== null) {
      extractAndAppend(el);
    }
  });
}

function __start(){
  extractAndAppend(main());
}
__start();
