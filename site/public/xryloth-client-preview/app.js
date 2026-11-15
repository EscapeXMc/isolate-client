// Isolate Client web preview — browser-only. Everything doable in-browser is live;
// native-only actions show the installer modal.
const page=document.getElementById('page'),modal=document.getElementById('modal'),mmsg=document.getElementById('modal-msg'),crumbCur=document.getElementById('crumb-cur'),crumbWrap=document.getElementById('crumb-cur-wrap');
const LABELS={home:'Home',xai:'Isolate ClientAI',accounts:'Login',settings:'Settings',instances:'Profiles',servers:'Servers',mods:'Mods',modpacks:'Modpacks',packs:'Resource Packs',chat:'Chat',templates:'Templates',themes:'Themes',logs:'Logs',configeditor:'Config Editor'};
document.getElementById('m-close').onclick=()=>modal.hidden=true;
document.getElementById('m-go').onclick=()=>open('/downloads','_blank');
modal.addEventListener('click',e=>{if(e.target===modal)modal.hidden=true});
const nx=document.getElementById('notice-x');if(nx)nx.onclick=()=>document.getElementById('notice').remove();
function need(m){mmsg.textContent=m||"This can't be done within the browser. Install the client to get full access.";modal.hidden=false}
const esc=s=>String(s).replace(/</g,'&lt;').replace(/>/g,'&gt;');
const fmtN=n=>n>=1e6?(n/1e6).toFixed(1)+'M':n>=1e3?(n/1e3).toFixed(1)+'K':String(n);

/* ---------- settings (persisted, applied live) ---------- */
const DEFSET={theme:'black',ram:'4G',username:'player',particles:true,transitions:true};
let SET={...DEFSET};
try{Object.assign(SET,JSON.parse(localStorage.getItem('isolate-preview')||'{}'))}catch(e){}
function saveSet(){try{localStorage.setItem('isolate-preview',JSON.stringify(SET))}catch(e){}}
function applySet(){
  applyTheme(SET.theme,true);
  document.getElementById('bg').style.display=SET.particles?'':'none';
  document.body.classList.toggle('notrans',!SET.transitions);
}

/* ---------- real client themes ---------- */
const THEMES=[
 ['black','Black (Default)','#f4f4f5','#000'],
 ['cartoon','Blue Cartoon','#1e88e5','#fff'],
 ['godred','God Red','#ff3366','#fff'],
 ['isolateclient','IsolateClient','#ff0022','#fff'],
 ['purple','Purple','#b847ff','#fff'],
 ['sapphire','Sapphire','#00aaff','#000'],
 ['midnight','Midnight','#00c8e8','#000'],
 ['lunar','Lunar','#00e6a0','#000'],
 ['feature','Feature','#ff4030','#fff'],
 ['greengradient','Green Gradient','#00e888','#000'],
 ['orange','Orange','#ff7a18','#000'],
];
function hexA(hex,a){return hex+a}
function applyTheme(id,silent){
  const t=THEMES.find(x=>x[0]===id)||THEMES[0];
  const r=document.documentElement.style;
  r.setProperty('--acc',t[2]);r.setProperty('--acc-soft',hexA(t[2],'22'));r.setProperty('--acc-text',t[3]);
  if(!silent){SET.theme=id;saveSet()}
}

/* ---------- real built-in templates ---------- */
const TPLS=[
 ['fabric-tech-reborn','Tech Reborn (Fabric)','Fabric','1.20.1','Automation and tech mods on Fabric — Tech Reborn, AE2, Mekanism, Pneumatic Craft',['Tech Reborn','Applied Energistics 2','Mekanism','PneumaticCraft','JEI']],
 ['fabric-ad-astra','Ad Astra (Fabric)','Fabric','1.20.1','Space exploration modpack — Ad Astra, rockets, VisIER and QoL mods',['Ad Astra','VisIER','Ad Astra Jetpacks','JEI','Fabric Language Kotlin']],
 ['fabric-create-astral','Create: Astral (Fabric)','Fabric','1.20.1','Create modpack with Astral and addons — Create, Astral, Steam n Rails',['Create','Create: Astral','Create: Steam n Rails','Create: Armory','JEI']],
 ['fabric-better-end','Better End (Fabric)','Fabric','1.20.1','Adventure and exploration — Better End, Better Nether, Biome Makeover',['Better End','Better Nether','Biome Makeover','JEI','Sodium','Iris Shaders']],
 ['fabric-chromatic-iron','Chromatic Iron (Fabric)','Fabric','1.20.1','Vanilla-friendly QoL — Chromatic Iron, REI, Appleskin, performance mods',['Chromaic Curse','Appleskin','JEI','Sodium','Lithium','Phosphor']],
 ['fabric-lets-do','Let\'s Do (Fabric)','Fabric','1.20.1','Cozy building and farming — Let\'s Do series with furniture and cooking',['Let\'s Do: Build','Let\'s Do: Meadow','Let\'s Do: Cooking','Let\'s Do: Beach Party','JEI']],
 ['fabric-vault-hunters','Vault Hunters (Fabric)','Fabric','1.20.1','Adventure RPG — Vault Hunters with quests, skills and epic loot',['Vault Hunters','JEI','Jade','Sodium','Iris Shaders']],
 ['fabric-skyfactory','SkyFactory (Fabric)','Fabric','1.20.1','Skyblock — start on a tree, build everything with Ex Nihilo',['Ex Nihilo: Sequentia','Ex Nihilo: MEK Add-on','JEI','Sodium']],
 ['forge-ftb-revelation','FTB Revelation (Forge)','Forge','1.20.2','All-purpose modpack — tech, magic and exploration',['FTB Revelation','Mekanism','Applied Energistics 2','Twilight Forest','JEI']],
 ['forge-ftb-direwolf20','FTB Direwolf20 (Forge)','Forge','1.20.2','Direwolf20 curated — tech, magic and everything between',['FTB Direwolf20','Mekanism','Botania','Twilight Forest','JEI']],
 ['forge-enigmatica-6','Enigmatica 6 (Forge)','Forge','1.16.5','Expert modpack — challenging recipes and progression',['Enigmatica 6','Mekanism','Botania','Astral Sorcery','Twilight Forest']],
 ['forge-rl','RLCraft (Forge)','Forge','1.12.2','Hardcore survival — realistic mechanics, tough mobs',['RLCraft','Lycanites Mobs','Tough As Nails','JEI']],
 ['forge-atm9','All the Mods 9 (Forge)','Forge','1.20.1','Massive 400+ mod pack — the ultimate kitchen-sink experience',['All the Mods 9','Mekanism','Applied Energistics 2','Botania']],
 ['forge-bettermc','Better MC (Forge)','Forge','1.16.5','Enhanced vanilla — improved worldgen and QoL',['Better MC','Biome Makeover','TerraBlender','JEI','Xaero Minimap']],
 ['vanilla-optimized','Optimized Vanilla (Fabric)','Vanilla','1.20.1','Vanilla + performance — Sodium, Iris, Lithium, Starlight',['Sodium','Iris Shaders','Lithium','Phosphor','Starlight']],
 ['vanilla-building','Building (Fabric)','Vanilla','1.20.1','Building tools — WorldEdit, Litematica, Create, gadgets',['WorldEdit','Litematica','Create','Building Gadgets','JEI']],
 ['vanilla-qol','Quality of Life (Fabric)','Vanilla','1.20.1','QoL essentials — JEI, Appleskin, Jade, Xaero maps',['JEI','Appleskin','Jade','Xaero Minimap','Sodium']],
 ['vanilla-shaders','Shaders (Fabric)','Vanilla','1.20.1','Visuals — Iris, Complementary Shaders, performance mods',['Iris Shaders','Complementary Shaders','Sodium','Lithium']],
 ['server-hardcore','Hardcore Server','Server','1.20.1','Hardcore survival server — single life, epic loot',['Hardcore Darkness','Infernal Mobs','JEI']],
 ['server-survival','Survival Server','Server','1.20.1','Classic survival server — exploration and minimap mods',['Biome Makeover','TerrariaCopy','JEI','Xaero Minimap']],
];

/* ---------- live Modrinth data (cached) ---------- */
const mcache={};
const FALLBACK_MODS=[['Sodium','FPS boost rendering','0.5.x'],['Lithium','Tick optimisation','0.11.x'],['Iris Shaders','Shader loader','1.6.x'],['FerriteCore','Memory reduction','6.x']];
async function modrinth(query,type){
  const key=type+'|'+query;
  if(mcache[key])return mcache[key];
  const url='https://api.modrinth.com/v2/search?query='+encodeURIComponent(query)+'&limit=12&index=relevance&facets='+encodeURIComponent(JSON.stringify([[`project_type:${type}`]]));
  const r=await fetch(url);
  if(!r.ok)throw new Error('HTTP '+r.status);
  const j=await r.json();
  mcache[key]=j.hits||[];
  return mcache[key];
}
function mcard(h,isPack){
  const v=(h.versions||[]).slice(-2).join(', ')||'—';
  return `<div class="gcard"><div class="mitem">${h.icon_url?`<img src="${esc(h.icon_url)}" alt="" loading="lazy" width="40" height="40">`:''}<div class="mi"><h4>${esc(h.title)}</h4><p>by ${esc(h.author)} · ${fmtN(h.downloads||0)} downloads</p></div></div><p style="margin-top:8px">${esc((h.description||'').slice(0,110))}</p><div style="margin-top:6px"><span class="tag">${esc(v)}</span> <span class="dl">${isPack?'modpack':'mod'}</span></div><div class="row2"><button class="mini go" data-act="native">Install</button><button class="mini" data-modrinth="${esc(h.slug)}">Details</button></div></div>`;
}
function bindDetails(scope){
  scope.querySelectorAll('[data-modrinth]').forEach(b=>b.onclick=()=>open('https://modrinth.com/project/'+b.dataset.modrinth,'_blank'));
}
async function liveList(el,query,type,isPack,fallback){
  el.innerHTML='<div class="spin2"></div>';
  const render=hits=>{el.innerHTML=hits.length?hits.map(h=>mcard(h,isPack)).join(''):'<div class="gcard"><p>No results. Try another search.</p></div>';bindDetails(el)};
  if(!navigator.onLine){render([]);return}
  try{render(await modrinth(query,type))}
  catch(e){el.innerHTML='<div class="gcard"><p>Live list unavailable offline — showing built-in picks.</p></div>'+fallback.map(m=>`<div class="gcard"><h4>${esc(m[0])}</h4><p>${esc(m[1])} · ${esc(m[2])}</p><div class="row2"><button class="mini go" data-act="native">Install</button></div></div>`).join('')}
}

/* ---------- demo community chat ---------- */
const CHAT_SEED=[['Steve','Anyone on 1.20.1 Fabric? Need help with Iris + Sodium combo'],['Alex','use Iris 1.6 + Sodium 0.5, works fine here'],['ModdedMike','Vault Hunters template is so good'],['Noor','how do I allocate more RAM?'],['Steve','Settings → RAM in the full client']];
const CHAT_REPLIES=['Nice!','Same setup here, runs great','Check the template browser in the full client','Sodium + Lithium fixed my lag','Welcome!'];
let chatInit=false;
function chatHTML(){return CHAT_SEED.map(m=>`<div class="cmsg"><span class="who">${esc(m[0])}</span>${esc(m[1])}</div>`).join('')}

/* ---------- icons ---------- */
const IC='<svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.7" style="vertical-align:-2px"><path d="M12 2l9 5-9 5-9-5z"/><path d="M3 12l9 5 9-5"/></svg>';
const PL='<svg viewBox="0 0 24 24" width="12" height="12" fill="currentColor" style="vertical-align:-1px"><path d="M7 4l13 8-13 8z"/></svg>';
const AV='<svg viewBox="0 0 24 24" width="30" height="30" fill="none" stroke="#fff" stroke-width="1.6"><circle cx="12" cy="8" r="4"/><path d="M4 21c0-4 3.6-6 8-6s8 2 8 6"/></svg>';
const INST=[{name:'Fabric 1.20.1',mc:'1.20.1',loader:'fabric'},{name:'Vanilla 1.20.1',mc:'1.20.1',loader:'vanilla'},{name:'Forge 1.19.2',mc:'1.19.2',loader:'forge'}];
const SRV=[{name:'Local SMP',motd:'Friends only',ver:'1.20.1',pl:'2/10'},{name:'Paper Test',motd:'Plugins test',ver:'1.20.4',pl:'0/20'}];

const V={
home(){return `<div class="hero"><div style="flex:1;min-width:260px"><p class="sub tinycaps">good evening, ${esc(SET.username)}</p><h1><span class="w">your </span><span class="w">adventure</span><br><span class="g">starts now</span></h1><p class="desc">3 epic Minecraft worlds await your command. Preview data — real stats live in the desktop client.</p><div class="btnrow"><button class="btn-p" data-act="native">+ New Instance</button><button class="btn-s" data-act="nav" data-to="settings">Customize</button><button class="btn-s" data-act="site" data-to="/downloads">Get the launcher</button></div></div><div class="pcard"><div class="who"><div class="avatar">${AV}</div><div><div style="font-weight:800">${esc(SET.username)}</div><div class="tinycaps" style="font-size:10px;color:var(--mut)">online · preview profile</div></div></div><div class="tinycaps" style="font-size:10px;color:var(--mut);margin:14px 0 8px">recent activity</div><div class="gcard" style="padding:10px">Login as ${esc(SET.username)} · Microsoft</div></div></div>
<div class="grid3"><div class="gcard"><h4 class="tinycaps">cpu usage</h4><p>Processor load</p><div class="big">12%</div></div><div class="gcard"><h4 class="tinycaps">memory</h4><p>Allocated ${esc(SET.ram)}</p><div class="big">3.4 GB</div></div><div class="gcard"><h4 class="tinycaps">disk space</h4><p>61% used</p><div class="big">128 GB</div></div></div>
<div class="grid3">${INST.map(i=>`<div class="gcard"><h4>${IC} ${i.name}</h4><p>MC ${i.mc} · ${i.loader}</p><div class="row2"><button class="mini go" data-act="native">${PL} Play</button><button class="mini" data-act="nav" data-to="instances">Open</button></div></div>`).join('')}</div>`},
instances(){return `<div class="btnrow"><button class="btn-p" data-act="native">+ New Instance</button><button class="btn-s" data-act="native">Import</button></div><div class="grid3">${INST.map(i=>`<div class="gcard"><h4>${IC} ${i.name}</h4><p>MC ${i.mc} · ${i.loader}</p><div class="row2"><button class="mini go" data-act="native">Play</button><button class="mini" data-act="native">Mods</button><button class="mini" data-act="native">Delete</button></div></div>`).join('')}</div>`},
servers(){return `<div class="btnrow"><button class="btn-p" data-act="native">+ Add Server</button></div><div class="grid3">${SRV.map(s=>`<div class="gcard"><h4>${s.name}</h4><p>${esc(s.motd)} · ${s.ver} · ${s.pl}</p><div class="row2"><button class="mini go" data-act="native">Join</button><button class="mini" data-act="native">Edit</button></div></div>`).join('')}</div>`},
mods(){return `<label>Search live Modrinth mods (real results)</label><input id="q" placeholder="sodium, iris..." autocomplete="off"><div class="grid3" id="mg"></div>`},
modpacks(){return `<label>Search live Modrinth modpacks (real results)</label><input id="q" placeholder="better mc, atm9..." autocomplete="off"><div class="grid3" id="mg"></div>`},
packs(){return `<label>Search live resource packs (real results)</label><input id="q" placeholder="bare bones, fresh animations..." autocomplete="off"><div class="grid3" id="mg"></div>`},
xai(){return `<div class="gcard"><h4 class="tinycaps">isolate ai</h4><p>Groq + Gemini run in the desktop client only.</p><div id="chat" style="margin-top:10px;display:flex;flex-direction:column;gap:8px"><div class="gcard">Hi, I'm Isolate Client AI (preview). Tool actions are disabled here.</div></div><div class="btnrow"><input id="in" placeholder="Ask something..." style="flex:1" autocomplete="off"><button class="btn-p" id="send">Send</button></div></div>`},
accounts(){return `<div class="gcard"><h4 class="tinycaps">login</h4><p>Microsoft / Ely.by / Offline — accounts live in the desktop client. Your preview name:</p><div class="btnrow"><input id="uname" value="${esc(SET.username)}" style="flex:1" maxlength="16"><button class="btn-p" id="usave">Save</button></div><div class="btnrow"><button class="btn-s" data-act="native">Add Microsoft Account</button><button class="btn-s" data-act="native">Skins</button></div></div>`},
chat(){return `<div class="gcard"><h4 class="tinycaps">chat</h4><p>Live demo room — messages stay in this tab. Full history + cross-server chat need the client.</p><div class="chatbox" id="cbox"></div><div class="btnrow"><input id="cin" placeholder="Message..." style="flex:1" maxlength="200" autocomplete="off"><button class="btn-p" id="csend">Send</button></div></div>`},
templates(){return `<label>Search 20 built-in templates</label><input id="q" placeholder="tech, skyblock, shader..." autocomplete="off"><div class="catrow" id="cats"></div><div class="grid3" id="tg"></div>`},
themes(){return `<p style="color:var(--mut);font-size:12px">Same 11 built-in themes as the desktop client — applied live to this preview.</p><div class="swatches" id="sw"></div>`},
logs(){return `<div class="gcard"><p>launcher.log · game.log — desktop only.</p><div class="btnrow"><button class="btn-s" data-act="native">Open Folder</button></div></div>`},
configeditor(){return `<div class="gcard"><p>server.properties editor — desktop only.</p><div class="btnrow"><button class="btn-p" data-act="native">Edit</button></div></div>`},
settings(){return `<div class="gcard"><h4 class="tinycaps">profile</h4><label>Username (preview only)</label><input id="s-user" value="${esc(SET.username)}" maxlength="16"><label>RAM allocation (preview only)</label><select id="s-ram">${['2G','4G','6G','8G','12G','16G'].map(r=>`<option ${SET.ram===r?'selected':''}>${r}</option>`).join('')}</select></div><div class="gcard" style="margin-top:12px"><h4 class="tinycaps">appearance</h4><label>Theme (works in preview)</label><select id="s-theme">${THEMES.map(t=>`<option value="${t[0]}" ${SET.theme===t[0]?'selected':''}>${esc(t[1])}</option>`).join('')}</select><div class="trow"><div>Background orbs<small>Animated background glow</small></div><button class="tgl ${SET.particles?'on':''}" id="s-part"></button></div><div class="trow"><div>Smooth transitions<small>Launcher-style motion (0.5s)</small></div><button class="tgl ${SET.transitions?'on':''}" id="s-trans"></button></div></div><div class="btnrow"><button class="btn-p" id="s-save">Save Settings</button><button class="btn-s" data-act="native">Java Path (desktop)</button></div><p style="color:var(--mut);font-size:11px;margin-top:10px">Launching, Java and file access need the desktop client.</p>`}
};

let navTimer=null;
function go(id){
  document.querySelectorAll('.sec button').forEach(b=>b.classList.toggle('active',b.dataset.page===id));
  crumbCur.textContent=LABELS[id]||id;crumbWrap.style.display=id==='home'?'none':'';
  clearTimeout(navTimer);page.classList.remove('enter');page.classList.add('leave');
  navTimer=setTimeout(()=>{
    page.classList.remove('leave');void page.offsetWidth;page.innerHTML=V[id]?V[id]():V.home();page.classList.add('enter');
    if(id==='mods'||id==='modpacks'||id==='packs'){
      const type=id==='mods'?'mod':id==='modpacks'?'modpack':'resourcepack';
      const mg=document.getElementById('mg'),q=document.getElementById('q');
      const load=v=>liveList(mg,v,type,type!=='mod',FALLBACK_MODS);
      load('');let t; q.oninput=e=>{clearTimeout(t);t=setTimeout(()=>load(e.target.value.trim()),400)};
    }
    if(id==='xai'){const s=document.getElementById('send');if(s)s.onclick=()=>{const inp=document.getElementById('in');const v=(inp.value||'').trim();if(!v)return;document.getElementById('chat').insertAdjacentHTML('beforeend',`<div class="gcard">Preview reply: tool actions disabled. <b>Install the client to get full access.</b></div>`);inp.value=''}}
    if(id==='accounts'){document.getElementById('usave').onclick=()=>{SET.username=document.getElementById('uname').value.trim()||'player';saveSet();go('accounts')}}
    if(id==='chat'){
      const box=document.getElementById('cbox');box.innerHTML=chatHTML();box.scrollTop=box.scrollHeight;
      const send=()=>{const inp=document.getElementById('cin');const v=inp.value.trim();if(!v)return;box.insertAdjacentHTML('beforeend',`<div class="cmsg me"><span class="who">you</span>${esc(v)}</div>`);inp.value='';box.scrollTop=box.scrollHeight;setTimeout(()=>{box.insertAdjacentHTML('beforeend',`<div class="cmsg"><span class="who">Isolate ClientBot</span>${CHAT_REPLIES[Math.floor(Math.random()*CHAT_REPLIES.length)]}</div>`);box.scrollTop=box.scrollHeight},1200)};
      document.getElementById('csend').onclick=send;document.getElementById('cin').onkeydown=e=>{if(e.key==='Enter')send()};
    }
    if(id==='templates'){
      const cats=['All','Fabric','Forge','Vanilla','Server'],box=document.getElementById('cats'),tg=document.getElementById('tg'),q=document.getElementById('q');
      let cat='All';
      const draw=()=>{box.innerHTML=cats.map(c=>`<button class="cat ${cat===c?'on':''}" data-cat="${c}">${c}</button>`).join('');
        const s=(q.value||'').toLowerCase();
        tg.innerHTML=TPLS.filter(t=>(cat==='All'||t[2]===cat)&&(!s||t[1].toLowerCase().includes(s)||t[4].toLowerCase().includes(s))).map(t=>`<div class="gcard"><h4>${esc(t[1])}</h4><p>${esc(t[4])}</p><div style="margin:6px 0"><span class="tag">${t[2]}</span> <span class="chip">MC ${t[3]}</span> <span class="chip">${t[5].length} mods</span></div><div>${t[5].map(m=>`<span class="chip">${esc(m)}</span>`).join('')}</div><div class="row2"><button class="mini go" data-act="native">Use Template</button></div></div>`).join('')||'<div class="gcard"><p>No templates match.</p></div>'};
      box.onclick=e=>{const b=e.target.closest('[data-cat]');if(b){cat=b.dataset.cat;draw()}};
      q.oninput=draw;draw();
    }
    if(id==='themes'){
      const sw=document.getElementById('sw');
      sw.innerHTML=THEMES.map(t=>`<button class="swatch ${SET.theme===t[0]?'on':''}" data-th="${t[0]}"><span class="dot" style="background:${t[2]}"></span>${esc(t[1])}</button>`).join('');
      sw.onclick=e=>{const b=e.target.closest('[data-th]');if(!b)return;applyTheme(b.dataset.th);sw.querySelectorAll('.swatch').forEach(x=>x.classList.toggle('on',x===b))};
    }
    if(id==='settings'){
      document.getElementById('s-theme').onchange=e=>applyTheme(e.target.value);
      document.getElementById('s-part').onclick=e=>{SET.particles=!SET.particles;e.target.classList.toggle('on',SET.particles);saveSet();applySet()};
      document.getElementById('s-trans').onclick=e=>{SET.transitions=!SET.transitions;e.target.classList.toggle('on',SET.transitions);saveSet();applySet()};
      document.getElementById('s-save').onclick=()=>{SET.username=document.getElementById('s-user').value.trim()||'player';SET.ram=document.getElementById('s-ram').value;saveSet();need('Settings saved in this browser tab. Launching and Java still need the desktop client.')};
    }
  },150);
}
document.addEventListener('click',e=>{const p=e.target.closest('[data-page]');if(p){go(p.dataset.page);return}const a=e.target.closest('[data-act]');if(!a)return;if(a.dataset.act==='nav'){go(a.dataset.to);return}if(a.dataset.act==='site'){open(a.dataset.to,'_blank');return}need()});
applySet();
go('home');
setTimeout(()=>{const s=document.getElementById('splash');if(s)s.classList.add('hide');setTimeout(()=>s.remove(),500)},900);
setInterval(()=>{if(modal.hidden){mmsg.textContent='Heads up — this is only a browser preview. The real Isolate Client launcher looks and runs better than this. Download the full client for the complete experience.';modal.hidden=false}},120000);
