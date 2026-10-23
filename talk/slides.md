---
marp: true
title: Recovery Phrase Backup
theme: default
paginate: true
style: |
  /* The website's terminal look: phosphor green on near-black, monospace, square edges. */
  section {
    --mono: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    --bg: #050806;
    --panel: #0a100c;
    --text: #c9f5d6;
    --muted: #5d8a6b;
    --border: #1c3a26;
    --accent: #3dff8f;
    --warn: #e8d36a;
    --fire: #ff7a45;
    --color-background: var(--bg);
    --color-background-code: var(--panel);
    --color-background-stripe: var(--panel);
    --color-border: var(--border);
    --color-foreground: var(--text);
    --color-highlight: var(--accent);
    background: var(--bg); color: var(--text); font-family: var(--mono);
    font-size: 26px; line-height: 1.6; padding: 60px 70px;
  }
  section::after { color: var(--muted); font-family: var(--mono); font-size: 20px; }
  h1, h2 { font-family: var(--mono); }
  h1 { color: var(--text); font-size: 52px; }
  h1::before { content: "> "; color: var(--accent); }
  h2 { color: var(--accent); font-size: 34px; text-transform: uppercase; letter-spacing: 0.06em; }
  strong, code { color: var(--accent); }
  code { background: none; font-family: var(--mono); }
  table th, table td { border: 1px solid var(--border); background: var(--bg); color: var(--text); }
  table tr:nth-child(2n) td { background: var(--panel); }
  table th { color: var(--accent); }
  table { font-size: 22px; }
  section.title p { color: var(--muted); }
  section.cover h1 { font-size: 64px; margin-bottom: 28px; }
  section.cover p { font-size: 34px; margin: 0; }
  section.cover p.lead { color: var(--muted); margin-bottom: 12px; }
  .cols.seedcols { grid-template-columns: 1.5fr 1fr; margin-bottom: 28px; }
  /* A recovery phrase on paper, like a wallet's backup card. */
  .seed { background: #f3efe4; color: #141414; padding: 18px 24px 14px; transform: rotate(-2deg); box-shadow: 0 8px 24px rgba(0, 0, 0, 0.6); }
  .seed .seed-title { font-family: var(--mono); font-size: 16px; letter-spacing: 0.08em; text-transform: uppercase; color: #555; margin-bottom: 8px; }
  .seed ol { columns: 2; column-gap: 28px; margin: 0; padding-left: 2.4em; font-family: var(--mono); font-size: 22px; line-height: 1.55; color: #141414; }
  .seed li::marker { color: #888; }
  .cols { display: grid; grid-template-columns: 1fr 1fr; gap: 40px; align-items: center; }
  .cols.wide { grid-template-columns: 2fr 1fr; }
  .cols.top { align-items: start; }
  .muted, .small { color: var(--muted); }
  .small { font-size: 20px; }
  .small.url { font-size: 18px; white-space: nowrap; }
  .warn { color: var(--warn); }

  /* The website's explainer: one 14 s loop. */
  svg.story { display: block; width: 840px; height: 420px; margin: 0 auto 16px; border: 1px solid var(--border); background: var(--panel); }
  .line { stroke: var(--accent); stroke-width: 2; fill: none; }
  .card rect { fill: var(--bg); stroke: var(--accent); stroke-width: 2; }
  .card line { stroke: var(--muted); stroke-width: 3; }
  .place { fill: none; stroke: var(--muted); stroke-width: 2; }
  .label { fill: var(--muted); font-family: var(--mono); font-size: 13px; text-anchor: middle; }
  .caption { fill: var(--text); font-family: var(--mono); font-size: 16px; text-anchor: middle; opacity: 0; }
  .flame { fill: var(--fire); }
  .tick { stroke: var(--accent); stroke-width: 4; fill: none; }
  .found text { fill: var(--warn); font-family: var(--mono); font-size: 22px; font-weight: 700; }
  .found circle, .found .handle { fill: none; stroke: var(--warn); stroke-width: 3; }
  .anim { transform-box: view-box; animation-duration: 14s; animation-iteration-count: infinite; animation-timing-function: ease-in-out; }
  #phrase { animation-name: phrase; }
  #tick { animation-name: tick; opacity: 0; }
  #share-left { animation-name: share-left; }
  #share-mid { animation-name: share-mid; }
  #share-right { animation-name: share-right; }
  #places { animation-name: places; opacity: 0; }
  #flame { animation-name: flame; opacity: 0; }
  #found { animation-name: found; opacity: 0; }
  #c1 { animation-name: c1; }
  #c2 { animation-name: c2; }
  #c3 { animation-name: c3; }
  #c4 { animation-name: c4; }
  #c5 { animation-name: c5; }
  @keyframes phrase { 0%, 14% { opacity: 1; } 20%, 57% { opacity: 0; } 64%, 93% { opacity: 1; } 97% { opacity: 0; } 100% { opacity: 1; } }
  @keyframes tick { 0%, 60% { opacity: 0; } 65%, 93% { opacity: 1; } 97%, 100% { opacity: 0; } }
  @keyframes share-left {
    0%, 14% { opacity: 0; transform: translate(190px, -78px); }
    18% { opacity: 1; transform: translate(190px, -78px); }
    25%, 38% { opacity: 1; transform: translate(0, 0); }
    48%, 93% { opacity: 0.18; transform: translate(0, 0); }
    97%, 100% { opacity: 0; transform: translate(0, 0); }
  }
  @keyframes share-mid {
    0%, 14% { opacity: 0; transform: translate(0, -78px); }
    18% { opacity: 1; transform: translate(0, -78px); }
    25%, 50% { opacity: 1; transform: translate(0, 0); }
    60% { opacity: 1; transform: translate(0, -78px); }
    65%, 100% { opacity: 0; transform: translate(0, -78px); }
  }
  @keyframes share-right {
    0%, 14% { opacity: 0; transform: translate(-190px, -78px); }
    18% { opacity: 1; transform: translate(-190px, -78px); }
    25%, 50% { opacity: 1; transform: translate(0, 0); }
    60% { opacity: 1; transform: translate(-190px, -78px); }
    65%, 100% { opacity: 0; transform: translate(-190px, -78px); }
  }
  @keyframes places { 0%, 25% { opacity: 0; } 32%, 93% { opacity: 1; } 97%, 100% { opacity: 0; } }
  @keyframes flame { 0%, 38% { opacity: 0; } 43%, 58% { opacity: 1; } 64%, 100% { opacity: 0; } }
  @keyframes found { 0%, 68% { opacity: 0; } 72%, 90% { opacity: 1; } 95%, 100% { opacity: 0; } }
  @keyframes c1 { 0%, 16% { opacity: 1; } 20%, 97% { opacity: 0; } 100% { opacity: 1; } }
  @keyframes c2 { 0%, 20% { opacity: 0; } 24%, 36% { opacity: 1; } 40%, 100% { opacity: 0; } }
  @keyframes c3 { 0%, 40% { opacity: 0; } 44%, 54% { opacity: 1; } 57%, 100% { opacity: 0; } }
  @keyframes c4 { 0%, 57% { opacity: 0; } 61%, 68% { opacity: 1; } 71%, 100% { opacity: 0; } }
  @keyframes c5 { 0%, 71% { opacity: 0; } 74%, 92% { opacity: 1; } 96%, 100% { opacity: 0; } }

  /* With reduced motion, each drawing holds one frame that tells its story. */
  @media (prefers-reduced-motion: reduce) {
    .anim, .anim2 { animation: none !important; }
    #c2, #places, #k5, #phrase2, #tick2 { opacity: 1; }
  }

  /* Groups: either 2 of 3 places or 5 of 7 people recover the phrase. One 18 s loop.
     Each moving card sets --dx/--dy, the offset from its place to the phrase. */
  .anim2 { transform-box: view-box; animation-duration: 18s; animation-iteration-count: infinite; animation-timing-function: ease-in-out; }
  .glabel { fill: var(--accent); font-family: var(--mono); font-size: 13px; text-anchor: middle; }
  .divider { stroke: var(--border); stroke-width: 1; stroke-dasharray: 4 4; }
  #k1 { animation-name: k1; }
  #k2 { animation-name: k2; }
  #k3 { animation-name: k3; }
  #k4 { animation-name: k4; }
  #k5 { animation-name: k5; }
  #g1 { animation-name: g1; }
  #g2 { animation-name: g2; }
  #phrase2 { animation-name: phrase2; opacity: 0; }
  #tick2 { animation-name: tick2; opacity: 0; }
  .join-a { animation-name: join-a; }
  .join-b { animation-name: join-b; }
  @keyframes k1 { 0%, 2% { opacity: 0; } 4%, 17% { opacity: 1; } 19%, 100% { opacity: 0; } }
  @keyframes k2 { 0%, 19% { opacity: 0; } 21%, 36% { opacity: 1; } 38%, 100% { opacity: 0; } }
  @keyframes k3 { 0%, 38% { opacity: 0; } 40%, 56% { opacity: 1; } 58%, 100% { opacity: 0; } }
  @keyframes k4 { 0%, 58% { opacity: 0; } 60%, 78% { opacity: 1; } 80%, 100% { opacity: 0; } }
  @keyframes k5 { 0%, 80% { opacity: 0; } 82%, 96% { opacity: 1; } 98%, 100% { opacity: 0; } }
  @keyframes g1 { 0% { opacity: 0; } 4%, 96% { opacity: 1; } 100% { opacity: 0; } }
  @keyframes g2 { 0%, 18% { opacity: 0; } 23%, 96% { opacity: 1; } 100% { opacity: 0; } }
  @keyframes phrase2 { 0%, 47% { opacity: 0; } 50%, 57% { opacity: 1; } 59%, 69% { opacity: 0; } 72%, 95% { opacity: 1; } 98%, 100% { opacity: 0; } }
  @keyframes tick2 { 0%, 49% { opacity: 0; } 51%, 57% { opacity: 1; } 59%, 71% { opacity: 0; } 73%, 95% { opacity: 1; } 98%, 100% { opacity: 0; } }
  @keyframes join-a {
    0%, 40% { opacity: 1; transform: translate(0, 0); }
    47% { opacity: 1; transform: translate(var(--dx), var(--dy)); }
    50% { opacity: 0; transform: translate(var(--dx), var(--dy)); }
    52% { opacity: 0; transform: translate(0, 0); }
    60%, 100% { opacity: 1; transform: translate(0, 0); }
  }
  @keyframes join-b {
    0%, 60% { opacity: 1; transform: translate(0, 0); }
    67% { opacity: 1; transform: translate(var(--dx), var(--dy)); }
    70% { opacity: 0; transform: translate(var(--dx), var(--dy)); }
    72% { opacity: 0; transform: translate(0, 0); }
    80%, 100% { opacity: 1; transform: translate(0, 0); }
  }
---

<!-- _class: cover -->
<!-- _paginate: false -->

# Recovery Phrase Backup

<p class="lead">Split your seed phrase into shares.</p>
<p>No single place can <strong>lose</strong> it.</p>
<p>No single place can <strong>reveal</strong> it.</p>

---

## One copy fails both ways

<div class="cols seedcols">
<div>

Your wallet comes down to 12 or 24 words.

- **Find it** and it's stolen
- **Burn it** and it's gone
- And your family has to be able to recover it if something happens to you

</div>
<div class="seed">
<div class="seed-title">Recovery phrase</div>
<ol>
<li>leopard</li><li>involve</li><li>vital</li><li>powder</li><li>diary</li><li>motion</li>
<li>comfort</li><li>foil</li><li>width</li><li>later</li><li>gasp</li><li>year</li>
</ol>
</div>
</div>

A backup that a burglar can't use, a fire can't destroy, and your heirs can still recover: getting all three right on your own is hard.

---

## The usual fixes move the risk

| | What's left |
| --- | --- |
| Steel plate | Survives the fire, not the burglar |
| BIP39 passphrase | A second secret that needs its own backup |
| Ledger Recover | Hands parts of your seed to companies |
| Shamir in a wallet (Trezor) | Inconvenient: for an existing seed, a new wallet and moving everything |

---

## Split it into shares

<svg class="story" viewBox="0 0 600 300" role="img" aria-label="A recovery phrase split into three shares, kept in three places: losing one loses nothing, any two recover the phrase, and one alone reveals nothing.">
<text id="c1" class="caption anim" x="300" y="26">Your recovery phrase: one copy.</text>
<text id="c2" class="caption anim" x="300" y="26">Split into three shares, kept in three places.</text>
<text id="c3" class="caption anim" x="300" y="26">One is lost in a fire. Nothing is lost.</text>
<text id="c4" class="caption anim" x="300" y="26">Any two give your phrase back.</text>
<text id="c5" class="caption anim" x="300" y="26">Someone who finds one share learns nothing.</text>
<g id="phrase" class="anim card">
<rect x="250" y="42" width="100" height="66"></rect>
<line x1="262" y1="58" x2="338" y2="58"></line>
<line x1="262" y1="70" x2="330" y2="70"></line>
<line x1="262" y1="82" x2="338" y2="82"></line>
<line x1="262" y1="94" x2="320" y2="94"></line>
</g>
<path id="tick" class="anim tick" d="M 358 70 l 10 10 l 20 -24"></path>
<g id="places" class="anim">
<path class="place" d="M 85 222 l 25 -22 l 25 22 M 92 216 v 30 h 36 v -30 M 104 246 v -14 h 12 v 14"></path>
<path class="place" d="M 278 210 h 44 v 36 h -44 z M 278 210 l 22 -12 l 22 12 M 286 218 v 22 M 300 218 v 22 M 314 218 v 22"></path>
<path class="place" d="M 490 218 m -9 0 a 9 9 0 1 0 18 0 a 9 9 0 1 0 -18 0 M 474 248 q 16 -26 32 0"></path>
<text class="label" x="110" y="272">home</text>
<text class="label" x="300" y="272">bank box</text>
<text class="label" x="490" y="272">a friend</text>
</g>
<g id="share-left" class="anim card">
<rect x="82" y="130" width="56" height="40"></rect>
<line x1="90" y1="142" x2="130" y2="142"></line>
<line x1="90" y1="152" x2="124" y2="152"></line>
<line x1="90" y1="162" x2="130" y2="162"></line>
</g>
<g id="share-mid" class="anim card">
<rect x="272" y="130" width="56" height="40"></rect>
<line x1="280" y1="142" x2="320" y2="142"></line>
<line x1="280" y1="152" x2="314" y2="152"></line>
<line x1="280" y1="162" x2="320" y2="162"></line>
</g>
<g id="share-right" class="anim card">
<rect x="462" y="130" width="56" height="40"></rect>
<line x1="470" y1="142" x2="510" y2="142"></line>
<line x1="470" y1="152" x2="504" y2="152"></line>
<line x1="470" y1="162" x2="510" y2="162"></line>
</g>
<path id="flame" class="anim flame" d="M 110 236 c -22 -8 -18 -34 -4 -48 c 0 12 8 14 10 6 c 4 -10 -2 -18 4 -26 c 12 16 18 30 12 48 c -2 10 -10 18 -22 20 z"></path>
<g id="found" class="anim found">
<g class="card">
<rect x="440" y="120" width="56" height="40"></rect>
<line x1="448" y1="132" x2="488" y2="132"></line>
<line x1="448" y1="142" x2="482" y2="142"></line>
<line x1="448" y1="152" x2="488" y2="152"></line>
</g>
<circle cx="510" cy="160" r="20"></circle>
<path class="handle" d="M 524 174 l 18 18"></path>
<text x="503" y="168">?</text>
</g>
</svg>

Shamir's Secret Sharing, here **2 of 3**. Also 3 of 5, or your own numbers.

---

## Groups: one for you, one for your heirs

<svg class="story" viewBox="0 0 600 300" role="img" aria-label="One backup in two groups: any 2 of 3 places, or any 5 of 7 friends and heirs. Either group alone recovers the phrase; fewer shares reveal nothing.">
<text id="k1" class="caption anim2" x="300" y="26">Group 1: any 2 of 3 places, for you.</text>
<text id="k2" class="caption anim2" x="300" y="26">Group 2: any 5 of 7 friends and heirs.</text>
<text id="k3" class="caption anim2" x="300" y="26">You recover it with 2 of your 3.</text>
<text id="k4" class="caption anim2" x="300" y="26">After you're gone, 5 of the 7 recover it together.</text>
<text id="k5" class="caption anim2" x="300" y="26">Either group alone is enough. Fewer reveal nothing.</text>
<g id="phrase2" class="anim2 card">
<rect x="260" y="44" width="80" height="52"></rect>
<line x1="270" y1="58" x2="330" y2="58"></line>
<line x1="270" y1="68" x2="324" y2="68"></line>
<line x1="270" y1="78" x2="330" y2="78"></line>
<line x1="270" y1="88" x2="316" y2="88"></line>
</g>
<path id="tick2" class="anim2 tick" d="M 350 70 l 8 8 l 16 -20"></path>
<g id="g1" class="anim2">
<text class="glabel" x="135" y="136">group 1: any 2 of 3</text>
<path class="place" transform="translate(-50 0)" d="M 85 222 l 25 -22 l 25 22 M 92 216 v 30 h 36 v -30 M 104 246 v -14 h 12 v 14"></path>
<path class="place" transform="translate(-165 0)" d="M 278 210 h 44 v 36 h -44 z M 278 210 l 22 -12 l 22 12 M 286 218 v 22 M 300 218 v 22 M 314 218 v 22"></path>
<path class="place" transform="translate(-280 0)" d="M 490 218 m -9 0 a 9 9 0 1 0 18 0 a 9 9 0 1 0 -18 0 M 474 248 q 16 -26 32 0"></path>
<text class="label" x="60" y="272">home</text>
<text class="label" x="135" y="272">bank box</text>
<text class="label" x="210" y="272">a friend</text>
<g class="card anim2 join-a" style="--dx: 240px; --dy: -94px"><rect x="40" y="150" width="40" height="28"></rect><line x1="46" y1="160" x2="74" y2="160"></line><line x1="46" y1="168" x2="68" y2="168"></line></g>
<g class="card anim2 join-a" style="--dx: 165px; --dy: -94px"><rect x="115" y="150" width="40" height="28"></rect><line x1="121" y1="160" x2="149" y2="160"></line><line x1="121" y1="168" x2="143" y2="168"></line></g>
<g class="card"><rect x="190" y="150" width="40" height="28"></rect><line x1="196" y1="160" x2="224" y2="160"></line><line x1="196" y1="168" x2="218" y2="168"></line></g>
</g>
<line class="divider" x1="270" y1="122" x2="270" y2="282"></line>
<g id="g2" class="anim2">
<text class="glabel" x="435" y="136">group 2: any 5 of 7</text>
<path class="place" transform="translate(-175 0)" d="M 490 218 m -9 0 a 9 9 0 1 0 18 0 a 9 9 0 1 0 -18 0 M 474 248 q 16 -26 32 0"></path>
<path class="place" transform="translate(-135 0)" d="M 490 218 m -9 0 a 9 9 0 1 0 18 0 a 9 9 0 1 0 -18 0 M 474 248 q 16 -26 32 0"></path>
<path class="place" transform="translate(-95 0)" d="M 490 218 m -9 0 a 9 9 0 1 0 18 0 a 9 9 0 1 0 -18 0 M 474 248 q 16 -26 32 0"></path>
<path class="place" transform="translate(-55 0)" d="M 490 218 m -9 0 a 9 9 0 1 0 18 0 a 9 9 0 1 0 -18 0 M 474 248 q 16 -26 32 0"></path>
<path class="place" transform="translate(-15 0)" d="M 490 218 m -9 0 a 9 9 0 1 0 18 0 a 9 9 0 1 0 -18 0 M 474 248 q 16 -26 32 0"></path>
<path class="place" transform="translate(25 0)" d="M 490 218 m -9 0 a 9 9 0 1 0 18 0 a 9 9 0 1 0 -18 0 M 474 248 q 16 -26 32 0"></path>
<path class="place" transform="translate(65 0)" d="M 490 218 m -9 0 a 9 9 0 1 0 18 0 a 9 9 0 1 0 -18 0 M 474 248 q 16 -26 32 0"></path>
<text class="label" x="435" y="272">friends and heirs</text>
<g class="card anim2 join-b" style="--dx: -15px; --dy: -92px"><rect x="302" y="152" width="26" height="20"></rect><line x1="307" y1="159" x2="323" y2="159"></line><line x1="307" y1="165" x2="319" y2="165"></line></g>
<g class="card anim2 join-b" style="--dx: -55px; --dy: -92px"><rect x="342" y="152" width="26" height="20"></rect><line x1="347" y1="159" x2="363" y2="159"></line><line x1="347" y1="165" x2="359" y2="165"></line></g>
<g class="card"><rect x="382" y="152" width="26" height="20"></rect><line x1="387" y1="159" x2="403" y2="159"></line><line x1="387" y1="165" x2="399" y2="165"></line></g>
<g class="card anim2 join-b" style="--dx: -135px; --dy: -92px"><rect x="422" y="152" width="26" height="20"></rect><line x1="427" y1="159" x2="443" y2="159"></line><line x1="427" y1="165" x2="439" y2="165"></line></g>
<g class="card anim2 join-b" style="--dx: -175px; --dy: -92px"><rect x="462" y="152" width="26" height="20"></rect><line x1="467" y1="159" x2="483" y2="159"></line><line x1="467" y1="165" x2="479" y2="165"></line></g>
<g class="card"><rect x="502" y="152" width="26" height="20"></rect><line x1="507" y1="159" x2="523" y2="159"></line><line x1="507" y1="165" x2="519" y2="165"></line></g>
<g class="card anim2 join-b" style="--dx: -255px; --dy: -92px"><rect x="542" y="152" width="26" height="20"></rect><line x1="547" y1="159" x2="563" y2="159"></line><line x1="547" y1="165" x2="559" y2="165"></line></g>
</g>
</svg>

Any **2 of your 3** places, or any **5 of 7** friends and heirs: either group alone recovers the phrase.

---

## A share card

![w:1000](share-card.png)

The fingerprint, 20 words, and a note for whoever keeps it. No address, no share number, no threshold.

---

## Why I built this

Tools for this (SLIP-39) exist. I wanted one that is:

- **Easy**: a wizard from your phrase to checked shares, a recovery test, and a sheet for your heirs
- **Nothing to install**: one HTML file with embedded WebAssembly. Double-click it, offline, on any computer, even a live USB stick
- **Future-proof**: an open standard, not my format. Keep the file next to your shares: browsers still open pages from the 1990s

---

## Two moments: backup and recovery

The backup can ask a lot of you. The recovery can't ask much of your heirs.

<div class="cols top">
<div>

**Backup**: once, by you

- You know what a seed phrase is
- A clean computer, offline
- Each copy checked, then a recovery test

</div>
<div>

**Recovery**: (hopefully) years later

- Maybe not technical, maybe grieving
- A "how to recover" sheet, a note on every card
- One file that still opens, and an open standard

</div>
</div>

---

## You don't have to trust me

- **Offline**: not a wallet, never touches a network. Ideally on a live USB stick
- **Reproducible build**, signed by **`heeckhau.eth`**: check that the file is built from the published source
- **Open source**, tested against other SLIP-39 tools: you don't even need this one to recover

---

<!-- _paginate: false -->

## Demo

![bg right:45% fit](../.github/screenshot.png)

The demo version comes with sample phrases:

<p class="small url">heeckhau.github.io/recovery-phrase-backup/demo.html</p>

**Never type your real seed into a computer that's online.**

---

<!-- _class: title -->
<!-- _paginate: false -->

## Try it

<div class="cols wide">
<div>

**heeckhau.github.io/recovery-phrase-backup**

github.com/heeckhau/recovery-phrase-backup

I'd love your feedback.

What's unclear?<br>What's missing?<br>What would stop you from using it?

</div>
<div>

![w:340](qr.svg)

</div>
</div>

