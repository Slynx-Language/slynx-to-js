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
