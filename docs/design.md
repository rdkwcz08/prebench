# Charte de design de Prebench

Prebench doit ressembler à un instrument posé sur une paillasse d'électronique, pas à une application web générique. Cette charte fait foi pour toute l'interface ; la maquette de référence est `docs/design/maquette-ecran-principal.html`.

## 1. Idée directrice

**La paillasse est le héros, tout le reste est discret.** Le canevas reprend le tapis antistatique (ESD) d'un vrai poste de travail : gris-bleu, quadrillé. Dessus, une carte dessinée à plat et des fils Dupont colorés qui retombent légèrement. Autour, une interface native, dense et calme, qui ne colore que ce qui porte une information.

Un seul élément mémorable : le tapis et ses fils. Tout le reste se fait oublier.

## 2. Couleurs

Toutes les couleurs passent par des variables CSS (`--pb-*`, définies dans `app/ui/app.css`). Aucune valeur en dur dans les composants. Deux thèmes, portés par `<html data-theme>` : par défaut ils suivent le réglage du système (`prefers-color-scheme`), en direct ; le bouton de thème de la barre d'outils fait défiler « système », « clair », « sombre », et le choix est mémorisé.

### 2.1 Interface

| Jeton | Rôle | Sombre | Clair |
| --- | --- | --- | --- |
| `--pb-chrome` | Fond des barres et panneaux | `#1E2124` | `#E9EAE7` |
| `--pb-panel` | Fond des zones de contenu (inspecteur, série) | `#25292D` | `#F6F6F4` |
| `--pb-line` | Bordures, séparateurs | `#383D42` | `#C9CDCB` |
| `--pb-text` | Texte principal | `#E3E5E2` | `#1E2124` |
| `--pb-text-quiet` | Texte secondaire | `#98A1A6` | `#5D666B` |
| `--pb-accent` | Sélection, focus, action principale | `#3FA7D6` | `#1C77A6` |
| `--pb-on-accent` | Texte sur fond `--pb-accent` (bouton principal) | `#10202A` | `#FFFFFF` |
| `--pb-shadow` | Ombre de ce qui flotte (menus, infobulles) | `0 4px 12px` noir 40 % | `0 4px 12px` noir 16 % |

L'accent est le bleu de la voie 2 d'un oscilloscope. Il sert à une seule chose : montrer ce qui est sélectionné ou ce qu'on peut faire. Jamais en décoration.

### 2.2 Paillasse

| Jeton | Rôle | Sombre | Clair |
| --- | --- | --- | --- |
| `--pb-mat` | Tapis ESD | `#2C454C` | `#B9CDD0` |
| `--pb-mat-grid` | Quadrillage du tapis (pas de 24 px, trait de 1 px) | `#365560` | `#A6BEC2` |
| `--pb-mat-ink` | Texte posé sur le tapis (repères des composants, état vide) | `#C9D8DB` | `#22383E` |
| `--pb-board` | Vernis de la carte | `#1F3B5C` | `#1F3B5C` |
| `--pb-copper` | Pastilles, broches | `#B87333` | `#B87333` |
| `--pb-silk` | Sérigraphie | `#E8ECEF` | `#E8ECEF` |

La carte et le cuivre ne changent pas avec le thème : ce sont des matériaux.

### 2.3 États logiques et sévérité

La couleur porte l'état, jamais seule : chaque état a aussi un mot ou une forme.

| Jeton | Sens | Sombre | Clair | Doublé par |
| --- | --- | --- | --- | --- |
| `--pb-high` | Niveau haut | `#E6B422` | `#9A7300` | « HAUT », trait plein |
| `--pb-low` | Niveau bas | `#6B7B84` | `#6B7B84` | « BAS » |
| `--pb-float` | Flottant | hachures `--pb-text-quiet` | idem | « flottant », hachures |
| `--pb-conflict` | Conflit, erreur | `#E5484D` | `#C62828` | « conflit », icône |
| `--pb-warn` | Avertissement | `#E79A3C` | `#A85B00` | icône triangle |
| `--pb-ok` | Prérequis présent, test réussi | `#4CB782` | `#1F7A4D` | coche |

### 2.4 Fils

Couleurs des fils Dupont, attribuées par défaut selon la fonction, modifiables par l'utilisateur :

| Fonction | Couleur |
| --- | --- |
| Alimentation (3V3, 5V) | rouge `#D9453B` |
| Masse | noir `#2A2A2A`, contour `--pb-gnd-outline` (`#8D9499` en sombre, `#2A2A2A` en clair) |
| Signal numérique | jaune `#E8C33A` |
| Analogique | vert `#3E9B5C` |
| Bus (I2C, SPI, UART) | bleu `#3D7FD1`, puis orange `#E07B39`, violet `#8A5CC7` |

### 2.5 Voies de mesure

Le traceur, l'analyseur logique et l'oscilloscope reprennent les couleurs de voies d'un oscilloscope : jaune `#E6B422`, cyan `#3FA7D6`, magenta `#D45AA8`, bleu `#5B7FE0`.

## 3. Typographie

- **Interface : police du système.** `-apple-system, BlinkMacSystemFont, "Segoe UI Variable", "Segoe UI", system-ui, sans-serif`. L'app est native : elle parle la langue visuelle de macOS et de Windows, et n'embarque aucune police.
- **Données : chasse fixe du système.** `ui-monospace, "SF Mono", "Cascadia Mono", Consolas, monospace`. Réservée à ce qui est réellement une donnée ou du code : noms de broches (`PA5`), valeurs mesurées, durées, sortie série, `fichier:ligne`, adresses I2C. Jamais pour un libellé d'interface.
- **Échelle** (px) : 11 (barre d'état), 12 (libellés secondaires), 13 (texte de base), 15 (titres de panneau), 18 (titre de dialogue). Pas d'autre taille.
- **Graisses** : 400 et 600 uniquement.
- **Casse** : casse de phrase partout (« Moniteur série », pas « MONITEUR SÉRIE » ni « Moniteur Série »). Aucun libellé en capitales.
- Chiffres tabulaires (`font-variant-numeric: tabular-nums`) pour toute valeur qui change.

## 4. Mise en page

```
┌──────────────────────────────────────────────────────────────┐
│ Barre d'outils : projet, carte, simulation, mode, vitesse     │ 40 px
├──────────────────────────────────────────────┬───────────────┤
│                                              │ Inspecteur    │
│   Paillasse (tapis ESD, carte, fils)         │ 280 px        │
│                                              │ repliable     │
├──────────────────────────────────────────────┴───────────────┤
│ Onglets : Série │ Traceur │ Analyseur │ Bus │ Avertissements  │ 220 px
│ contenu du panneau                                            │ repliable
├──────────────────────────────────────────────────────────────┤
│ Barre d'état : état, durée, CPU, RAM, priorité, build         │ 24 px
└──────────────────────────────────────────────────────────────┘
```

- Grille de 4 px. Espacements autorisés : 4, 8, 12, 16, 24.
- Alignement à gauche partout, sauf les valeurs numériques, alignées à droite dans les tableaux.
- Densité d'outil professionnel : hauteur de ligne de liste 24 px, champs et boutons 28 px.
- Panneaux séparés par des bordures de 1 px, sans ombre. L'ombre est réservée à ce qui flotte : menus, palette, infobulles.
- Rayons : 3 px pour les contrôles, 6 px pour les menus et dialogues, 0 pour les panneaux ancrés. Pas un rayon unique partout.

## 5. Composants

| Composant | Règle |
| --- | --- |
| Bouton principal | Fond `--pb-accent`, texte blanc ou `--pb-chrome`, un seul par zone |
| Bouton secondaire | Transparent, bordure `--pb-line` |
| Bouton icône | 28 × 28, infobulle obligatoire avec le raccourci clavier |
| Bouton icône indisponible avec une raison | Même aspect qu'un bouton désactivé (opacité 45 %), mais survolable et focalisable (`aria-disabled`) : l'infobulle ajoute, sous le libellé, une phrase en `--pb-text-quiet` qui dit pourquoi et quoi faire (« Indisponible pendant le débogage : … Relancez la session de débogage ») ; le clic est sans effet |
| Onglet | Texte `--pb-text-quiet`, onglet actif en `--pb-text` souligné de 2 px `--pb-accent` |
| Pastille d'état | Fond teinté à 15 % de la couleur d'état, texte de la couleur d'état, mot explicite |
| Bannière | Bandeau pleine largeur sous la barre d'outils, icône + phrase + action, jamais de dialogue bloquant pour une information |
| Champ | 28 px, bordure `--pb-line`, focus en contour 2 px `--pb-accent` |
| Liste de propriétés (inspecteur) | Libellé à gauche en `--pb-text-quiet`, valeur à droite, en chasse fixe si c'est une donnée |
| Infobulle | Fond `--pb-panel`, bordure `--pb-line`, rayon 6 px, ombre `--pb-shadow`, 12 px ; libellé puis touche du raccourci dans un cadre en chasse fixe ; apparaît après 400 ms au survol, aussitôt au focus clavier, sans animation |
| Menu flottant | Ouvert depuis un sélecteur de la barre d'outils (projet) : fond `--pb-panel`, rayon 6 px, ombre `--pb-shadow`, sections séparées par une bordure de 1 px ; Échap ou un clic en dehors le referme |
| Sélecteur | Bouton sans bordure de 28 px (icône, valeur, chevron), bordure `--pb-line` au survol, comme le sélecteur de projet de la maquette |
| Bascule de panneau | Bouton icône dont le libellé dit l'action (« Masquer l'inspecteur ») ; pas de teinte d'accent, puisqu'il ne montre pas une sélection |
| Bannière (sévérité) | Liseré gauche de 3 px et icône de la couleur de sévérité : `--pb-accent` pour une information, `--pb-warn`, `--pb-conflict` ; fond `--pb-panel` |
| Indicateur d'état (barre d'état) | Pastille ronde de 8 px suivie du mot : `--pb-ok` en cours, `--pb-warn` en pause ou « Arrêté sur un point d'arrêt », `--pb-low` prête, arrêtée ou « Alimentation coupée », anneau `--pb-text-quiet` en compilation ou démarrage, anneau `--pb-accent` « En attente du débogueur », `--pb-conflict` en erreur |
| Mesure de la barre d'état (CPU, RAM) | Libellé en texte secondaire, valeur en chasse fixe et chiffres tabulaires, largeur fixe alignée à droite (« CPU 100 % », « RAM 2,6 Mo », une décimale sous 10 Mo) ; affichée seulement pendant une simulation ; infobulle native qui rappelle la limite |
| Garde-fou (bannière) | Pause automatique : bannière `--pb-warn`, phrase qui dit ce qui s'est passé et quoi corriger, bouton secondaire « Reprendre » (le bouton principal de la barre d'outils dit aussi « Reprendre ») ; arrêt pour mémoire : bannière `--pb-conflict`, bouton « Fermer » |
| Contrôle segmenté (mode) | Édition / Simulation dans la barre d'outils : boutons accolés de 28 px, segment actif teinté à 18 % de `--pb-accent`, texte `--pb-text` en 600 ; touche `E` |
| Paillasse : déplacement et zoom | Molette ou pavé tactile : déplacement ; Ctrl/Cmd + molette ou pincement : zoom (0,5 à 2,5) autour du pointeur ; le quadrillage du tapis suit. En mode Édition, Espace + glisser déplace la vue ; en mode Simulation, Espace met en pause (infobulle du bouton Pause) |
| Mode Édition de la paillasse | Visible sans lire la barre d'outils : cadre intérieur 2 px `--pb-accent`, étiquette « Édition » (fond `--pb-accent`, texte `--pb-on-accent`, 12 px, coin haut-gauche), quadrillage plus marqué (`--pb-mat-grid` mêlé à 22 % de `--pb-mat-ink`). Le tapis garde sa couleur : il ne doit pas se confondre avec le thème |
| Ajuster la vue | Au chargement d'un projet, la vue cadre la carte et les composants avec une marge de 48 px (zoom au plus 150 %) ; bouton icône « Ajuster la vue » (⌘0 / Ctrl+0), ⌘1 / Ctrl+1 pour 100 %, double-clic sur le tapis vide |
| Composant en édition | Aimanté au pas de 24 du tapis ; sélection : contour 2 px `--pb-accent` décalé de 4 px ; vue inerte (on déplace, on n'actionne pas) |
| Broches et câblage | Broches des composants visibles en mode Édition (pastille cuivre). Pendant le tracé : broches compatibles remplies à 35 % et entourées de `--pb-accent`, autres atténuées à 30 % ; fil en cours en tirets `--pb-accent` ; broche sélectionnée entourée de `--pb-accent` 2 px. Échap abandonne, Supprimer retire la sélection |
| Fil | Contour sombre 6 px (couleur à 60 % sur noir ; masse : `--pb-gnd-outline`) puis âme 4 px de la couleur 2.4, retombée légère sous la corde, embout Dupont noir à chaque extrémité ; sélection : halo 10 px `--pb-accent` sous le fil ; couleur choisie dans l'inspecteur (« Auto » = selon la fonction) |
| États des nets sur les pastilles | Pendant la simulation, pastilles des broches utilisées (câblées ou configurées par le firmware), 9 px : haut et bas pleins (`--pb-high`, `--pb-low`), tirés haut et bas en contour 2 px de la même couleur, flottant hachuré (`--pb-silk` sur vernis, contour `--pb-text-quiet`), conflit plein `--pb-conflict` avec une croix. Le mot (Haut, Bas, Tiré haut, Tiré bas, Flottant, Conflit) au survol et en pastille d'état dans l'inspecteur |
| Pastille PWM et ADC | Sortie PWM : jauge horizontale, fond `--pb-low`, part haute `--pb-high` de gauche à droite selon le rapport cyclique, contour `--pb-high` ; entrée ADC : jauge verticale, même couleurs, remplie du bas vers le haut selon la valeur brute. Au survol : « PWM 20 kHz, 50 % », « ADC 1,65 V ». Seuls la fréquence et le rapport transitent : la jauge ne clignote jamais |
| Onglet Avertissements | Deux groupes titrés en 12 px `--pb-text-quiet` : « Câblage » (validations sans simulation, bouton texte « Afficher » en `--pb-accent` qui sélectionne la broche), puis « Simulation ». Une ligne de 24 px par entrée : icône (`--pb-warn` triangle, `--pb-conflict` cercle barré) et phrase qui dit quoi faire. Pastille de l'onglet : total, en `--pb-conflict` s'il y a un conflit ou un court-circuit |
| Nom affiché d'un composant | Repère sous le composant : nom affiché, sinon l'identifiant. Double-clic en Édition : champ en place (28 px, `--pb-panel`, focus `--pb-accent`), Entrée valide, Échap annule ; aussi « Nom affiché » dans l'inspecteur, avec le rappel de l'identifiant (en chasse fixe) que gardent les fils |
| Note sur la paillasse | Texte libre posé sur le tapis : 13 px, graisse 600, `--pb-mat-ink`, filet gauche de 2 px `--pb-mat-ink` à 50 %, 240 px de large au plus, ni fond ni ombre. Visible dans les deux modes ; en Édition, déplacée (aimantée au pas de 24), modifiée au double-clic, supprimée ; ajoutée par le bouton icône « Ajouter une note » ou un clic droit sur le tapis vide. Sélection : contour `--pb-accent` |
| Composant en attente | Ajouté pendant une simulation : hachures `--pb-mat-ink` à 45 % (forme) et « actif au prochain démarrage » sous son repère (mot) ; bannière d'information avec « Redémarrer pour l'activer » |
| Onglet avec pastille | Pastille de comptage `--pb-warn` par défaut ; `--pb-conflict` quand elle compte des erreurs (onglet Build) |
| Liste d'erreurs de build | Une phrase de résumé (« Build en échec : 2 erreurs, 1 avertissement. ») puis une ligne de 24 px par entrée : icône de sévérité (`--pb-conflict` erreur, `--pb-warn` avertissement), emplacement `fichier:ligne:colonne` en chasse fixe et `--pb-accent` (lien vers l'éditeur, souligné au survol), message du compilateur en chasse fixe ; un échec affiche l'onglet et déplie le panneau |
| État du build (barre d'état) | « Build en cours », « Build sim 1,2 s » (durée en chasse fixe), ou icône et « Build en échec » en `--pb-conflict` |
| Repère de composant | Sous le composant : `--pb-mat-ink` 12 px graisse 600 sur le tapis, `--pb-silk` 11 px sur la carte |
| Reset et alimentation (barre d'outils) | Après le bouton principal, comme dans la maquette : « Réinitialiser » (icône `reset`, touche R), puis bascule « Couper l'alimentation » / « Rétablir l'alimentation » (icône `power`, enfoncée et teintée d'accent tant que l'alimentation est coupée, pas de raccourci pour éviter une coupure par mégarde), puis « Arrêter la simulation ». Pendant une session de débogage, reset et coupure sont indisponibles avec une raison. Aucun des deux n'entre dans l'historique d'édition |
| Alimentation coupée | Barre d'état « Alimentation coupée » et message qui dit comment relancer ; LED éteintes et niveaux retombés sur la paillasse (la paillasse elle-même ne change pas d'aspect) ; repère « Alimentation coupée », puis « Alimentation rétablie », dans le Journal et le moniteur série ; champ d'envoi série désactivé |
| Repère du moniteur série | Ligne à part, police de l'interface en `--pb-text-quiet` (maquette : `.log .sys`), dans chaque port ; ignorée par le traceur ; exportée entre crochets |
| Inspecteur (broche, fil, composant) | Titre, sous-titre, puis trois sections Configuration, État, Net (maquette). Broche : mode, fonction (« GPIO », « USART2 TX ») et tirage réglés par le firmware, tirage soudé sur la carte, capacités ; niveau en pastille et période mesurée ; membres du net et fils. Fil : extrémités et couleur ; niveau du net ; membres. Composant : type, identifiant, paramètres, position ; état du modèle et niveau de chaque broche ; liaisons de chaque broche. Données en chasse fixe ; niveau inconnu en mot (« Hors simulation », « Non utilisé », « Hors tension ») ; ligne « Fil » précédée d'un échantillon 10 × 10 de sa couleur, puis « Jaune, signal ». Section État d'une sortie PWM : « Fréquence » (« 20 kHz ») et « Rapport cyclique » (« 50 % ») à la place de la période ; d'une entrée ADC : « Tension » (« 1,65 V ») et « Valeur ADC » (« 2 048 / 4 095 ») à la place du niveau logique ; fonction « PWM TIM2_CH1 » ou « ADC1 CH0 ». Composant : mesures des broches de la carte reliées, suffixées du repère (« Valeur ADC (A0) ») ; état du modèle en français (« Angle 90° », « Luminosité 50 % », valeurs comme « Anode commune ») |
| Paramètres d'un composant | En Édition, section « Paramètres » de l'inspecteur : un champ par paramètre du manifeste (liste pour un choix, nombre en chasse fixe avec son unité, case à cocher), 28 px, libellé au-dessus ; chaque modification entre dans l'historique d'édition et part au modèle en cours. Absente en Simulation |
| Vues PWM et ADC | Potentiomètre (maquette POT1) : bouton dont le trait va de −135° à +135°, glissé ou clavier (rôle slider ; flèches 1 %, Maj 10 %, Début et Fin). Servo : palonnier orienté selon l'angle, sans animation. Buzzer : ondes et fréquence en chasse fixe 11 px à l'encre du tapis quand il sonne. LED RGB : teinte mélangée, halo de 80 ms comme la LED |
| Outils de l'onglet Série | À droite de la barre d'onglets, comme dans la maquette : sélecteur « Port série » (une entrée par UART du firmware, « USART2, 115 200 bauds », plus « Sortie standard (printf) »), sélecteur « Débit du moniteur » (absent pour la sortie standard), puis boutons icône bascules « Afficher l'horodatage » et « Vue hexadécimale » (pressés : teinte d'accent), « Exporter la sortie… » (fenêtre « Enregistrer sous » du système, ouverte dans le dossier du projet avec le nom `serie-USART2-AAAAMMJJ-HHMMSS.txt` proposé ; points de suspension : le bouton ouvre une fenêtre), « Effacer ». Sélecteurs de 24 px en chasse fixe, bordure `--pb-line` |
| Moniteur série | Sortie en chasse fixe 12 px, lignes de 20 px, suit la fin tant qu'on n'est pas remonté ; horodatage `00:12.345` en `--pb-text-quiet` à gauche ; vue hexadécimale `48 65 6C` par morceau reçu. Ligne d'envoi en bas (UART seulement) : champ en chasse fixe, sélecteur « Fin de ligne » (LF, CR+LF, CR, Aucune), bouton secondaire « Envoyer » ; Entrée envoie |
| Alerte de débit | Débit du moniteur différent de celui du firmware : ligne en tête de la sortie, icône `--pb-warn` et phrase qui donne le bon débit ; recopiée dans l'onglet Avertissements, groupe « Simulation ». Le texte reste lisible (Prebench ne brouille pas les octets) |
| Traceur | Lignes « clé:valeur » du port choisi (ou nombres séparés par des espaces) : une courbe par nom, 8 au plus, 600 points. Couleurs de voies 2.5 dans l'ordre ; de la 5ᵉ à la 8ᵉ, mêmes couleurs en tirets (forme). Légende à gauche (180 px) : trait de la voie, nom, dernière valeur en chasse fixe et chiffres tabulaires ; axes gauche et bas 1 px `--pb-line`, valeurs extrêmes en 11 px `--pb-text-quiet`, ligne médiane en tirets `--pb-line`. Échelle verticale automatique |

Icônes : un seul jeu au trait (1,5 px), 16 px dans les barres. Symboles électroniques dessinés à la main dans le même style pour les composants. Aucun émoji.

## 6. Mouvement

Le mouvement ne montre qu'un changement d'état réel :

- une LED s'allume avec un halo de 80 ms, s'éteint sans transition ;
- un panneau s'ouvre ou se replie en 120 ms ;
- un fil se pose avec une courte retombée.

Rien d'autre ne bouge. `prefers-reduced-motion` supprime halos et transitions.

## 7. Écriture

- Français, casse de phrase, verbes d'action : « Démarrer », « Mettre en pause », « Réinitialiser », « Couper l'alimentation », « Créer le projet ».
- Une action garde son nom de bout en bout : le bouton « Démarrer » produit l'état « En cours ».
- Une erreur dit ce qui s'est passé et comment corriger, sans s'excuser : « CMake introuvable. Installez-le avec la commande ci-dessous, puis cliquez sur Revérifier. »
- Un état vide invite à agir : « Aucun composant. Ajoutez-en un depuis la barre d'outils. »
- Vocabulaire de l'électronicien : broche, net, niveau haut, rapport cyclique. Pas de jargon interne (acteur, Worker, shim) dans l'interface.

## 8. Interdits

- Dégradés décoratifs, effets de verre dépoli, néons.
- Cartes arrondies identiques avec ombre grise pour découper le contenu.
- Libellés en capitales, sur-titres au-dessus de chaque bloc.
- Chaînes de métadonnées séparées par des points médians (« A · B · C »).
- Flèche « → » ajoutée au texte des boutons.
- Émojis comme icônes.
- Couleur sans signification : toute couleur renvoie à la section 2.
- Police web embarquée pour l'interface.
- Animations d'entrée sur les panneaux, survols animés sur chaque élément.

## 9. Accessibilité

- Contraste d'au moins 4,5:1 pour le texte, 3:1 pour les icônes et bordures utiles.
- Tout est utilisable au clavier, focus toujours visible.
- Aucun état porté par la couleur seule (section 2.3).
- Taille minimale d'une cible cliquable : 24 × 24.
