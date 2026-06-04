function __init_Background(p0){
    return {f0: p0};
}
function __apply_Background(p0,p1){
    p0.style.backgroundColor = '#'+p1.f0.toString(16);
    return ;
}
function __init_BgGreen(p0){
    return {f0: __init_Background(((p0&255)<<8)).f0};
}
function __apply_BgGreen(p0,p1){
    p0.style.backgroundColor = '#'+p1.f0.toString(16);
    return ;
}
function main(){
    return Teste();
}
function _init_Aba(p0){
    p0.textContent = "Abababa";
    return ;
}
function _init_Teste(p0){
    __apply_BgGreen(p0,__init_BgGreen(127));
    let v0 = document.createElement("p");
    v0.textContent = "Jorge";
    __apply_Background(v0,__init_Background(16711680));
    p0.appendChild(v0);
    return ;
}
function Aba(){
    let c1 = document.createElement("p");
    _init_Aba(c1);
    return {c1};
}
function Teste(){
    let c1 = document.createElement("div");
    let c2 = Aba();
    _init_Teste(c1);
    return {c1,c2};
}
function buildDOM(parent, child) {
  for (const value of Object.values(child)) {
    if (value instanceof HTMLElement) {
      parent.appendChild(value);
    } else {
      buildDOM(parent, value);
    }
  }
}

function _start() {
  buildDOM(document.body, main());
}
_start();
