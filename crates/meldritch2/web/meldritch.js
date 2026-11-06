(() => {
  const controls = document.querySelector('#controls'), status = document.querySelector('#connection'), title = document.querySelector('#title');
  const login = document.querySelector('#login'), tokenInput = document.querySelector('#token');
  let socket, revision = -1, token = sessionStorage.getItem('meldritch-token') || '';
  const send = (message) => socket?.readyState === WebSocket.OPEN && socket.send(JSON.stringify(message));
  function render(snapshot) {
    if (snapshot.revision < revision) return;
    revision = snapshot.revision; title.textContent = snapshot.state.name; controls.replaceChildren();
    Object.values(snapshot.state.controls).forEach(c => {
      const box = document.createElement('section'); box.className = `control ${c.type}`; box.dataset.name = c.name;
      const label = document.createElement('label'); label.textContent = c.label; box.append(label);
      let input;
      if (c.type === 'continuous') { input = document.createElement('input'); input.type='range'; input.min=c.min; input.max=c.max; input.step=(c.max-c.min)/1000; input.value=c.value; input.oninput=()=>send({type:'control.set',name:c.name,value:Number(input.value)}); }
      else if (c.type === 'toggle') { input=document.createElement('button'); input.textContent=c.value?'ON':'OFF'; input.onclick=()=>send({type:'control.set',name:c.name,value:!c.value}); }
      else if (c.type === 'choice') { input=document.createElement('select'); c.choices.forEach(v=>input.add(new Option(v,v))); input.value=c.value; input.onchange=()=>send({type:'control.set',name:c.name,value:input.value}); }
      else { input=document.createElement('button'); input.textContent=c.name; input.onclick=()=>send({type:'control.set',name:c.name,value:true}); }
      box.append(input); controls.append(box);
    });
  }
  async function connect() {
    const config = await fetch('/api/browser-config').then(response=>response.json());
    if(!config.websocket){status.textContent='WebSocket disabled';return;}
    socket = new WebSocket(`${location.protocol === 'https:' ? 'wss' : 'ws'}://${location.host}${config.websocket}`);
    socket.onopen=()=>{status.textContent='connected'; if(token) send({type:'authenticate',token});};
    socket.onmessage=e=>{const m=JSON.parse(e.data); if(m.type==='state.snapshot') render(m);};
    socket.onclose=()=>{status.textContent='reconnecting'; setTimeout(connect,750);};
  }
  document.querySelector('#login-button').onclick=()=>{tokenInput.value=token;login.showModal();};
  login.addEventListener('close',()=>{if(login.returnValue==='join'){token=tokenInput.value;sessionStorage.setItem('meldritch-token',token);send({type:'authenticate',token});}});
  connect();
})();
