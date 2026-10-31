# Feuille de route

meshcore-m5 est un client MeshCore en terminal, relié en Bluetooth à un
ELECROW ThinkNode M5 (firmware MeshCore Companion Bluetooth v1.18.0).

## Fait

- [x] Connexion Bluetooth au M5, reconnexion automatique, instance unique
- [x] Messages directs et canal public, avec accusés de réception
- [x] Historique des messages sur disque (JSONL), écrit avant l'affichage
- [x] Liste des contacts et fiche du node : joignabilité, route, ping
- [x] Fiche du M5 : batterie (courbe LiPo), réglages radio, durée de
  connexion
- [x] Carte OpenStreetMap avec cache des tuiles, zoom et déplacement
- [x] Icônes de map.meshcore.io, épingle sur le node sélectionné
- [x] Apparences Normale et Hacker, choisies au menu de démarrage
- [x] Mode console Linux (`--console` ou `TERM=linux`) : affichage ASCII et
  radar de lettres à la place de la carte
- [x] Carte ajustée à la taille du panneau ; sélectionner le M5 montre ses
  voisins
- [x] Correctifs de meshcore-rs 0.2.0 (copie dans `vendor/`) : une seule
  connexion D-Bus, plus de notifications en double après une reconnexion

## À vérifier sur le matériel

- [ ] Ping d'un répéteur avec connexion invité (échec avant la correction
  sur "33LPOR Pier Ré")
- [ ] Mode console sur une vraie console Linux (TTY), puis via SSH
- [ ] Ajustement de la carte quand on sélectionne le M5
- [ ] Pourcentage de batterie comparé à celui de l'application Android

## Support de Meshtastic

Le M5 accepte les deux firmwares. Le programme choisirait le bon protocole
tout seul, selon ce que le M5 annonce en Bluetooth.

- [ ] Choisir la licence : la crate `meshtastic` 0.1.9 est en GPL-3.0 alors
  que le projet est en MIT. Passer le projet en GPL-3.0 ou réserver
  Meshtastic à une option de compilation
- [ ] Vérifier la licence des définitions Protobuf de Meshtastic
- [ ] Lire le code de la crate : Bluetooth sous Linux, connexion D-Bus
  ouverte à chaque connexion (risque de notifications en double)
- [ ] Détecter le firmware pendant la recherche Bluetooth, par l'UUID du
  service annoncé : MeshCore `6e400001-...`, Meshtastic `6ba1b218-...`
- [ ] Module `src/radio/meshtastic/` qui produit les mêmes `FromRadio` et
  reçoit les mêmes `ToRadio` que la partie MeshCore
- [ ] Nodes et positions tirés de la base de nodes de Meshtastic
- [ ] Messages : canal 0 pour le public, messages directs, accusés
- [ ] Batterie : pourcentage donné directement par le firmware
- [ ] Ping : traceroute à la place du statut des répéteurs
- [ ] Types de nodes : rôles Meshtastic (CLIENT, ROUTER...) dans la fiche
  et sur la carte
- [ ] Firmware détecté affiché dans l'en-tête
- [ ] Tests des conversions Meshtastic vers nos types
- [ ] Essai sur le M5 reflashé en Meshtastic

## Documentation et publication

- [ ] README : installation, appairage (`bluetoothctl` avec l'agent
  `KeyboardOnly` et le PIN affiché sur le M5), utilisation, touches
- [ ] Fichier LICENSE (le `Cargo.toml` annonce MIT)
- [ ] Proposer nos correctifs Bluetooth au projet meshcore-rs, pour pouvoir
  supprimer `vendor/`
- [ ] Tag v0.1.0 et notes de version
- [ ] Intégration continue GitHub Actions : tests et clippy

## Petites dettes connues

- [ ] La position du M5 n'est lue qu'à la connexion : elle ne suit pas un
  déplacement
- [ ] Les tuiles sorties de la vue restent dans la file de téléchargement
- [ ] Les réponses de connexion invité ne nomment pas le node : un seul
  ping à la fois
- [ ] Nombre maximal de contacts non affiché : meshcore-rs plafonne la
  valeur du firmware à 255
- [ ] Les chemins à hachage long s'affichent octet par octet
- [ ] Le défilement de la conversation suppose au plus 4 lignes par message
- [ ] Le type de node passe de `app` à `ui` par une chaîne ("repeater",
  "room"...)
- [ ] L'heure est lue de trois façons différentes (chrono `Utc`, `Local`,
  `SystemTime`)
- [ ] Le calcul des dossiers XDG est répété dans trois fichiers
