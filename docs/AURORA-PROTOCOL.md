# Protocole 3rd Party d'IVAO Aurora — notes de projet

## Activation côté Aurora

Le contrôleur doit activer l'accès dans Aurora : **F7 → Other → 3rd Party Software Access**. Sans ça, rien n'écoute sur le port et toute tentative de connexion échoue immédiatement.

## Connexion

- Aurora écoute en local uniquement : `127.0.0.1:1130` (TCP).
- Un logiciel tiers se connecte comme un client socket classique, aucune authentification.
- Encodage ASCII. Chaque message (requête ou réponse) est une ligne terminée par `\r\n`.
- Format d'une ligne : `#COMMANDE;champ1;champ2;...` — champs séparés par `;`, certains vides (`;;`).
- Le protocole n'a pas de corrélation requête/réponse explicite (pas d'ID de message) : on identifie la réponse à une requête par son nom de commande en tête de ligne. Si plusieurs requêtes de la même commande sont en vol simultanément, elles sont supposées répondre dans l'ordre d'envoi (file FIFO par commande — voir `AuroraClient.request()`).
- Aucun keepalive/heartbeat observé côté serveur : la détection de déconnexion se fait via l'événement `close` du socket TCP.

## Commandes utilisées dans ce projet

| Commande | Sens | Réponse |
|---|---|---|
| `#CONN` | Qui suis-je (station du contrôleur connecté à Aurora) | `#CONN;CALLSIGN` |
| `#SELTFC` | Indicatif de l'avion actuellement sélectionné dans Aurora | `#SELTFC;CALLSIGN` (vide si rien n'est sélectionné) |
| `#FP;CALLSIGN` | Plan de vol déposé d'un trafic | voir champs ci-dessous |
| `#TRPOS;CALLSIGN` | Position/état temps réel d'un trafic | voir champs ci-dessous |
| `#TRPATHL;CALLSIGN` | Route restante développée (SID/STAR/airways résolus en points), avec ETO | `FIX:ETO` répétés |
| `#TRPATHA;CALLSIGN` | Même format que `#TRPATHL`, mais inclut aussi les points déjà survolés (ETO à `-`) | `FIX:ETO` répétés |
| `#TR` | Liste brute des indicatifs actuellement visibles (radar) | liste d'indicatifs |
| `#ATC` | Liste des positions ATC en ligne | `STATION:FREQ` répétés |
| `#ATCT` | Existe côté Aurora (même format que `#ATC`), jamais utilisée dans ce projet | — |

`%SELTFC%` peut remplacer un indicatif dans n'importe laquelle de ces commandes (ex. `#TRPOS;%SELTFC%`) : Aurora substitue l'avion actuellement sélectionné. Voir "Pièges" ci-dessous — ce raccourci n'est pas sans risque.

## `#FP` — champs (plan de vol)

Indices vérifiés sur données réelles (2026-07-22) :

```
0  callsign
1  dep
2  arr
3  alternate
4  eobt
5  aircraft (type ICAO)
6  wake (categorie de turbulence de sillage : L/M/H/J...)
7  rules        (I / V / Y / Z)
8  flightType   (S / N / G / M / X)
9  equipment
10 cruiseLevel  (ex. "F330")
11 cruiseSpeed  (ex. "N0450")
12 endurance
13 eet
14 route        (texte libre du plan de vol depose)
15 remarks
```

⚠️ La doc officielle **inverse** les champs 7 (`rules`) et 8 (`flightType`) — vérifié faux sur données réelles, à ne pas suivre.

## `#TRPOS` — champs (position/état temps réel)

```
0  callsign
1  heading       (cap, degres)
2  track         (route sol, degres)
3  altitude      (pieds)
4  groundSpeed   (noeuds — vitesse SOL, pas indiquee)
5  lat
6  lon
7  squawkSet     (transpondeur affiche)
8  squawkLabel
9  wpLabel       (etiquette "prochain point" — voir Pieges, sert a detecter un direct)
10 altLabel      (niveau autorise inscrit sur l'etiquette Aurora par le controleur)
11 spdLabel
12 assumedBy     (station qui a assume le trafic ; vide si personne)
13 nextStation
14 onGround      ("1"/"0")
15 isSelected    ("1"/"0")
16 wasSelected   ("1"/"0")
17 gate         (poste actuel — voir "Postes de stationnement (gates)" plus bas)
18 voice
19 (non documente, jamais exploite dans ce projet)
20 verticalSpeed (ft/min — non documente officiellement ; > 0 montee, < 0 descente)
21 assignedGate  (poste ATTRIBUE, distinct du champ 17 — voir "Postes de stationnement (gates)")
```

⚠️ La doc officielle s'arrête au champ 18. Les champs 19 à 21 n'y figurent pas du tout — le champ 20 (vertical speed) a été identifié et confirmé par recoupement sur données réelles, le champ 21 (assignedGate) est décrit plus bas mais n'a pas été vérifié ici, le champ 19 reste de sens inconnu.

## `#TRPATHL` / `#TRPATHA` — route restante

Une ligne `#TRPATHx;CALLSIGN;FIX1:ETO1;FIX2:ETO2;...`. Aurora développe lui-même les SID/STAR/airways en points nommés individuels — pas besoin de les redévelopper côté client. `ETO` vaut `-` pour un point déjà survolé (uniquement visible via `#TRPATHA`, jamais via `#TRPATHL`).

Point notable : Aurora transmet parfois un point de virage **sans nom** (fix vide/blanc) au milieu de la séquence, typiquement juste avant un aéroport — jamais signalé dans la doc, découvert en écartant un bug réel où le "dernier point connu" tombait sur ce blanc plutôt que sur le vrai dernier point nommé.

## Postes de stationnement (gates)

⚠️ **Section non vérifiée ici.** Rien de ce qui suit n'est utilisé ni n'a été retesté dans ce projet : à traiter comme des pistes documentées, à revérifier sur capture réelle avant de coder dessus (cf. « Fiabilité de la documentation officielle » plus bas).

**Deux champs `#TRPOS` distincts concernent le poste** (voir la table de champs plus haut) :
- champ **17 (`gate`)** — le poste où l'avion se trouve **physiquement**, tel qu'affiché par Aurora. Vide tant qu'aucun poste n'a été déterminé.
- champ **21 (`assignedGate`)** — le poste **attribué**, potentiellement différent du 17 (avion pas encore arrivé à son poste, ou posé au mauvais poste). Absent de la doc officielle (qui s'arrête au champ 18, cf. plus haut).

⚠️ Constat important (non revérifié ici) : le champ 17 n'est renseigné par Aurora **que pour l'aéroport dont le plan de parkings est chargé/contrôlé activement** par la position connectée. Sur un aéroport que la position ne contrôle pas, le champ reste vide même si l'avion y est visiblement posé (`onGround` à `1`).

**Commandes liées** (présentes côté Aurora, non testées ici) :

| Commande | Sens | Réponse |
|---|---|---|
| `#BAY` | Demande la liste des offres/attributions de poste en cours (baylist) | `#BAY;Record1;Record2;...` — records séparés par `;`, champs internes séparés par `\|` : `Sender\|Receiver\|Callsign\|Text1\|Text2\|Time\|State` (State : `0`=créé non envoyé, `1`=offre/révision, `2`=accepté, `3`=rejeté) |
| `#LBGTE` | Étiquette de poste affichée pour un trafic | `#LBGTE;CALLSIGN;GATE` |

⚠️ Écart doc officielle (même famille que `@ERR` ci-dessous) : quand la baylist est vide, Aurora répond `@BAY;No data in bay` — préfixe `@` non documenté pour cette commande, au lieu du `#BAY;...` attendu. Le préfixe `@` semble donc servir aussi à des réponses "vides/informatives", pas seulement aux erreurs.

Aucune commande d'**écriture** d'attribution de poste n'a été identifiée avec certitude dans le protocole (le mécanisme d'offre/révision du baylist, mentionné par la doc officielle via des commandes `offer`/`revise`/`accept`/`reject` envoyées en PM, n'a jamais été testé en conditions réelles).

## Messages d'erreur

Le serveur répond `@ERR;#COMMANDE;ARGUMENT;raison` (préfixe `@`, parfois `$`) quand une commande est refusée. Le premier champ après `@ERR` nomme précisément la commande fautive — utile pour rejeter la bonne requête en attente plutôt que la plus ancienne de la file.

## Pièges rencontrés en session (aucun documenté officiellement)

1. **`#TRPOS;%SELTFC%` ferme la socket** si rien n'est sélectionné dans Aurora au moment de l'appel — jamais mentionné nulle part, découvert en observant des déconnexions inexpliquées. Parade : toujours vérifier avec `#SELTFC` seul (qui ne casse rien) avant d'appeler `#TRPOS;%SELTFC%`.
2. **`%SELTFC%` n'est pas indispensable pour interroger un avion précis** : on avait initialement cru qu'Aurora rejetait tout indicatif explicite hors sélection (d'où son usage systématique au début du projet) — retesté en session, `#FP;CALLSIGN` / `#TRPOS;CALLSIGN` / `#TRPATHA;CALLSIGN` fonctionnent très bien sur un avion non sélectionné. Le raccourci `%SELTFC%` ne reste utile que pour le mode "un seul avion suivi, celui sélectionné à l'écran".
3. **`wpLabel` (champ 9 de `#TRPOS`)** affiche soit un libellé de procédure ("SIDNAME PISTE", ex. `"BODRU8A 04R"`, toujours deux mots séparés par un espace), soit — quand un contrôleur envoie un trafic en direct sur un point — le nom du point tout seul, sans espace (ex. `"MTL"`). C'est la seule façon observée de détecter un "direct" posé manuellement par un contrôleur ; rien d'autre dans le protocole ne le signale explicitement.
4. **Champs numérotés au-delà de ce que documente la doc officielle** existent et portent de vraies données exploitables (vertical speed) — toujours vérifier un champ inconnu sur un échantillon réel avant de l'ignorer.

## Fiabilité de la documentation officielle

Constat du projet, à prendre au sérieux avant d'étendre l'usage du protocole : la doc officielle IVAO a déjà montré au moins 2 champs inversés, plusieurs champs non documentés porteurs de vraies données, et une contrainte de fonctionnement majeure (`#TRPOS;%SELTFC%`) absente de toute documentation. **Ne jamais faire confiance à la doc seule** — toujours vérifier un nouveau champ ou une nouvelle commande contre une capture réelle avant de coder dessus.

Pour ça, `mock-aurora.js` (à la racine du projet) rejoue des échantillons de session réelle capturés — l'outil le plus fiable pour tester une hypothèse sur le protocole sans avoir besoin d'Aurora ouvert et d'un contrôleur connecté.
