# IPEL / IHRA — Požiadavky na server

> **Minimálna** = akceptovateľné na spustenie. **Optimálna** = odporúčaná produkcia. **Expanzná** = rezerva pre rast bez novej VM.

---

## Spoločná platforma (všetky projekty)

Platí pre každú službu nižšie, pokiaľ projektová sekcia neuvádza inak.

### Operačný systém hostiteľa

| | |
|---|---|
| **OS** | Debian 14 (amd64) |
| **Nasadenie** | Docker (+ Docker Compose) na projekt |
| **TLS** | Univerzitný certifikát alebo Let's Encrypt na reverse proxy |
| **Verejné porty** | 443 HTTPS (80 → presmerovanie voliteľné) |
| **Porty aplikácie** | Bind len na `127.0.0.1`; **nevystavovať** verejne |

### Debian balíky (host)

**Runtime (každá VM):**

```bash
apt-get update
apt-get install -y --no-install-recommends \
  ca-certificates \
  curl \
  docker.io \
  docker-compose-plugin
```

**Voliteľné (prevádzka / CI deploy cieľ):**

```bash
apt-get install -y --no-install-recommends \
  git \
  rsync \
  unattended-upgrades
```

Pri vloženej SQLite netreba na hoste samostatný DB server. DB balíky inštalovať len tam, kde je uvedené (IPEL Rust optimálna/expanzná).

### Reverse proxy (na hostname)

| Požiadavka | Detail |
|------------|--------|
| **WebSocket** | Povinné — podpora upgrade na projektovo špecifických cestách |
| **Cieľ proxy** | `http://127.0.0.1:<port>` |
| **WS timeout** | ≥ 3600 s (dlhé relácie) |
| **Kompresia** | Voliteľný gzip/brotli pre statické assety |

### Autentifikácia

| | |
|---|---|
| **Uni OAuth / SSO** | Povinné ako **možnosť prihlásenia** na IPEL projektoch (GameDev, Rust) |
| **Lokálne účty** | Môžu zostať ako záloha, kde už sú implementované |
| **Prístup admin / dev** | Len SSH verejný kľúč pre správcov (`[SSH kľúče tímu u IT]`) |

### Prístup pre správcov

| Položka | Požiadavka |
|---------|------------|
| **SSH** | Prihlásenie kľúčom pre deploy používateľa(ov); žiadne zdieľané heslá |
| **Deploy user** | Vyhradený používateľ v skupine `docker`, napr. `ipel-deploy` |
| **Sudo** | Len `docker compose` + cesty k volume, alebo rootless Docker podľa pravidiel uni |
| **Tajomstvá** | OAuth client ID/secret, DB heslá, deploy kľúče — uni secret store / env súbory na serveri, nie v gite |

### CI/CD (všetky projekty)

| Položka | Detail |
|---------|--------|
| **Zdroj** | GitHub (`github.com/matusem/…`) |
| **Spúšťač** | Push na `main` → build image → deploy na cieľovú VM |
| **Registry** | `[ghcr.io / uni registry]` |
| **Deploy** | SSH → `docker compose pull && docker compose up -d` |
| **Perzistentné dáta** | Pomenované Docker volumes; pri redeploy **nikdy nemazať** |
| **Výpadok** | Krátky reštart kontajnera; aktívne WebSockety sa odpoja |

### Zálohovanie (všetky projekty)

| | |
|---|---|
| **Frekvencia** | Minimálne denne |
| **Retencia** | `[7 / 14 / 30]` dní |
| **Rozsah** | Všetky perzistentné volumes + compose/env konfigurácia |
| **Test obnovy** | Raz pred spustením do produkcie |

### Legenda veľkostí

| Úroveň | Význam |
|--------|--------|
| **Minimálna** | Spustenie / jedna cohorta kurzu |
| **Optimálna** | Odporúčaná stabilná produkcia |
| **Expanzná** | Väčšie cohorty, viac súbežných úloh alebo druhý kurz bez novej architektúry |

---

## IPEL GameDev — `gdd.ics.upjs.sk`

Multiplayer webová herná platforma. Autoritatívna herná logika vo WASM na serveri; lobby a klienti v prehliadači.

| | Minimálna | Optimálna | Expanzná |
|---|-----------|-----------|----------|
| **Používatelia** | 100 | 200 | 500 |
| **Súbežné hry** | 25 | 50 | 120 |
| **Peak WebSockets** | ~150 | ~300 | ~700 |
| **vCPU** | 2 | 4 | 8 |
| **RAM** | 8 GB | 16 GB | 32 GB |
| **Disk** | 20 GB SSD | 40 GB SSD | 80 GB SSD |
| **Databáza** | SQLite (súborový volume) | SQLite | SQLite; migrácia na Postgres len pri multi-instance |
| **Inštancie** | 1 | 1 | 1 (single-instance dizajn) |

```
Služba:         IPEL GameDev — multiplayer webová herná platforma
URL:            https://gdd.ics.upjs.sk/
Repozitár:      github.com/matusem/ipel-gamedev (framework/)
Nasadenie:      1× Docker kontajner
OAuth:          Uni SSO ako možnosť prihlásenia (povinné)
WebSockety:     /game, /graphql (lobby subscriptions)
Databáza:       SQLite — vložená, bez samostatného DB servera
Škálovanie:     Len jedna inštancia (herné relácie v pamäti)
Záloha volumes: /app/data (SQLite), /app/games (publikované hry)
Kontakt:        [kontakt dev tímu]
```

**WebSocket cesty:** `/game`, `/graphql`

**Balíky na hoste:** len spoločný runtime zoznam (žiadne extra DB balíky).

**Poznámky pre adminov:**

- Netreba PostgreSQL/Redis. Krátky výpadok pri deployi je akceptovateľný.
- **RAM:** každá aktívna hra drží Wasmtime inštanciu (~15–40 MB) plus JSON stav a WebSocket buffery; 50 súbežných hier plus lobby traffic a Docker overhead vyžaduje pri optimálnej úrovni **16 GB**, nie 8 GB.

---

## IPEL Rust — `uwu.ics.upjs.sk`

Gamifikovaný portál pre kurz Rustu: študenti prepoja **vlastné Git repozitáre** (GitHub / GitLab); platforma **zobrazuje commity** a spúšťa **automatické testy** na zvolenom commite. Body a rebríčky. **Zdrojový kód študentov sa na našich serveroch neukladá** — len URL repa, metadata commitov a výsledky testov.

| | Minimálna | Optimálna | Expanzná |
|---|-----------|-----------|----------|
| **Študenti** | 100 | 200 | 500 |
| **Súbežné test runy** | 5 | 15 | 40 |
| **Test runy / deň** | ~200 | ~500 | ~1500 |
| **vCPU** | 4 | 8 | 16 |
| **RAM** | 8 GB | 16 GB | 32 GB |
| **Disk** | 30 GB SSD | 60 GB SSD | 100 GB SSD |
| **Databáza** | PostgreSQL 16+ (malá) | PostgreSQL 16+ | PostgreSQL + voliteľná read replica |
| **Inštancie** | 1 app + 1 DB (alebo DB na tej istej VM) | 1 app + samostatná DB VM | App + worker VM(y) + DB VM |

```
Služba:         IPEL Rust — gamifikovaný portál cvičení z Rustu
URL:            https://uwu.ics.upjs.sk/
Repozitár:      [github.com/matusem/ipel-rust]
Nasadenie:      Docker Compose (app + PostgreSQL; voliteľný worker kontajner)
OAuth:          Uni SSO (povinné) + Git provider OAuth pre prístup k repu (GitHub/GitLab)
WebSockety:     [napr. /ws — stav test runu, aktualizácie rebríčka]
Git:            Kód študentov na ich Git účtoch; platforma klonuje len pri teste
Databáza:       PostgreSQL (používatelia, skóre, URL rep, commit SHA, výsledky testov — nie zdroják)
Škálovanie:     Horizontálni workeri pre test queue (optimálna+); DB oddelená od app
Záloha volumes: Len PostgreSQL dáta (žiadny archív odovzdaní/zdrojákov)
Kontakt:        [kontakt dev tímu]
```

**WebSocket cesty:** `[TBD — napr. /ws, /graphql]`

**Extra Debian balíky (ak PostgreSQL na tom istom hoste):**

```bash
# Len ak DB beží na hoste namiesto v kontajneri — preferovať DB v Compose
apt-get install -y --no-install-recommends postgresql-client
```

**Očakávané Compose služby:** `app`, `postgres`, voliteľný `worker` (dočasný `git clone` → `cargo test` → zmazať workspace).

**Poznámky pre adminov:**

- Náročnejší ako GameDev: **CPU-bound** (Rust compile + test na beh).
- Worker **naklonuje repozitár na commit**, spustí testy v sandboxe, **nepersistuje kód študenta** — disk je hlavne OS, Docker, DB a dočasná build cache.
- Outbound **HTTPS na GitHub/GitLab** potrebné pre klony a commit API.
- Sandboxované spúšťanie pravdepodobne vyžaduje **Docker-in-Docker** alebo vyhradeného workera s limitmi.
- Uni SSO + Git OAuth + PostgreSQL sú tvrdé požiadavky pre produkciu.

---

## IHRA — `ihra.ics.upjs.sk`

Modernizovaný IHRA web: verejná stránka, **upload hier** pre súťaž, **validácia** proti mnohým projektovým šablónam (~rovnaká veľkosť cohorty ako GameDev).

| | Minimálna | Optimálna | Expanzná |
|---|-----------|-----------|----------|
| **Používatelia / súťažiaci** | 100 | 200 | 400 |
| **Súbežné uploady / validácie** | 5 | 15 | 30 |
| **Šablóny** | ~10 | ~30 | ~60 |
| **vCPU** | 2 | 4 | 8 |
| **RAM** | 4 GB | 8 GB | 16 GB |
| **Disk** | 30 GB SSD | 60 GB SSD | 120 GB SSD |
| **Databáza** | SQLite alebo PostgreSQL | PostgreSQL | PostgreSQL |
| **Inštancie** | 1 | 1 | 1 app + voliteľný validation worker |

```
Služba:         IHRA — súťažný web a portál na odovzdávanie hier
URL:            https://ihra.ics.upjs.sk/
Repozitár:      [github.com/matusem/ihra]
Nasadenie:      1× Docker kontajner (Compose: app + DB pri PostgreSQL)
OAuth:          Voliteľné pri štarte; uni SSO odporúčané
WebSockety:     Voliteľné — [progress validácie / admin dashboard]
Databáza:       SQLite (minimálna) alebo PostgreSQL (optimálna+)
Škálovanie:     Jedna app inštancia; async validation queue pri špičkách
Záloha volumes: DB volume, /uploads alebo ekvivalent úložiska odovzdaní
Kontakt:        [kontakt dev tímu]
```

**WebSocket cesty:** `[TBD — voliteľné]`

**Balíky na hoste:** spoločný runtime zoznam; pridať `postgresql-client` pri externom Postgres.

**Poznámky pre adminov:**

- Upload + validácia šablón — podobný traffic profil ako GameDev uploady.
- WASM/validácia šablón môže vyžadovať **WebAssembly runtime** v kontajneri (žiadne extra knižnice na hoste).
- Termíny súťaže spôsobujú **upload špičky** — skoky disku a CPU; preferovať optimálnu úroveň.

---

## Súhrnná matica (pre IT ticket)

Skopírovať jeden blok na VM / požiadavku služby.

### GameDev — optimálna

```
Hostname:       gdd.ics.upjs.sk
Projekt:        IPEL GameDev
OS:             Debian 14 amd64
CPU / RAM / Disk: 4 vCPU, 16 GB RAM, 40 GB SSD
Databáza:       SQLite (vložená, perzistentný volume)
Docker:         áno
HTTPS + WSS:    áno
OAuth:          uni SSO povinné
SSH:            kľúče správcov, deploy user v docker skupine
CI/CD:          GitHub → SSH deploy
Záloha:         denne, volumes: app/data, app/games
```

### GameDev — minimálna (pilot)

```
Hostname:       gdd.ics.upjs.sk
Projekt:        IPEL GameDev
OS:             Debian 14 amd64
CPU / RAM / Disk: 2 vCPU, 8 GB RAM, 20 GB SSD
Databáza:       SQLite (vložená, perzistentný volume)
Poznámka:       ≤25 súbežných hier; pred plnou cohortou upgrade na optimálnu úroveň
```

### Rust — optimálna

```
Hostname:       uwu.ics.upjs.sk
Projekt:        IPEL Rust
OS:             Debian 14 amd64
CPU / RAM / Disk: 8 vCPU, 16 GB RAM, 60 GB SSD
Databáza:       PostgreSQL 16+ (perzistentný volume alebo samostatná DB VM)
Docker:         áno (app + postgres [+ worker])
HTTPS + WSS:    áno
OAuth:          uni SSO + Git provider (GitHub/GitLab) pre študentské repá
Git:            kód študentov sa neukladá; klon len pri teste
SSH:            kľúče správcov, deploy user v docker skupine
CI/CD:          GitHub → SSH deploy
Záloha:         denne, len PostgreSQL
```

### IHRA — optimálna

```
Hostname:       ihra.ics.upjs.sk
Projekt:        IHRA modernizácia
OS:             Debian 14 amd64
CPU / RAM / Disk: 4 vCPU, 8 GB RAM, 60 GB SSD
Databáza:       PostgreSQL (alebo SQLite pre minimálny pilot)
Docker:         áno
HTTPS + WSS:    áno (WSS voliteľné)
OAuth:          uni SSO odporúčané
SSH:            kľúče správcov, deploy user v docker skupine
CI/CD:          GitHub → SSH deploy
Záloha:         denne, DB + upload storage
```

---

## Otvorené body

- [ ] OAuth provider: `[URL uni IdP, proces registrácie klienta]`
- [ ] TLS: `[uni CA vs Let's Encrypt]`
- [ ] Reverse proxy: `[existujúci uni proxy / Nginx / Caddy na VM]`
- [ ] Container registry: `[ghcr.io / harbor.uni.sk / …]`
- [ ] Alokácia VM: `[jedna VM na projekt vs zdieľaný host]`
- [ ] IPEL Rust — URL repozitára, Git provider OAuth (GitHub/GitLab), politika worker/sandbox
- [ ] IHRA — URL repozitára a počet šablón pri štarte
- [ ] Retencia záloh a off-site kópia
- [ ] Staging hostnames: `[gdd-staging.…, uwu-staging.…, ihra-staging.…]`
