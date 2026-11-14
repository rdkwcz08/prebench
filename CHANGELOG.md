# Journal des modifications

Les évolutions de Prebench qui comptent pour les utilisateurs sont consignées ici.

Format inspiré de [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/). Versions [SemVer](https://semver.org/lang/fr/) en `0.x` jusqu'à la stabilité. `release.sh` exige une section `## [X.Y.Z]` avant de publier une version.

## [Non publié]

## [0.2.2] - 2026-11-14

**Aperçu technique** : simulation visible moins coûteuse, pour tenir les budgets de la section 11.1.

### Modifié

- Rendu de la simulation encore allégé : pastilles et vues des composants dans leurs propres couches, niveaux et broches mis à jour un par un et seulement s'ils changent (banc WKWebView, `tests/budget` : de 25 % à 17 % d'un cœur).
- Cadence d'affichage de la simulation : 15 images par seconde par défaut, appliquée par le cœur (une seule mise à jour par intervalle, qui regroupe tout ce qui a changé). Réglable à 60, 30 ou 15 (`frame_rate` dans `settings.toml`, pris en compte au démarrage d'une simulation) ; sur batterie, la plus basse de cette cadence et de celle sur batterie.

## [0.2.1] - 2026-11-14

**Aperçu technique** : simulation plus sobre. Mesurée sur Mac avec `tests/budget`, la 0.2.0 dépassait le budget de la section 11.1 (60 % d'un cœur en simulation, pour 10 % visés) : le rendu de la paillasse est allégé.

### Modifié

- Nouvelle icône de l'app. Sur macOS 26 et plus, elle suit l'apparence choisie (claire, sombre, transparente, teintée) ; icône fixe sur les macOS antérieurs et sous Windows.
- Une seule mise à jour de l'interface par image du cœur (états, niveaux, broches, sortie série), au lieu de quatre événements séparés ; seules les broches dont la configuration change sont envoyées.
- LED : le halo de 80 ms ne joue plus qu'à l'allumage ; un fondu PWM s'affiche sans transition (il n'y a plus d'animation permanente).
- Moniteur série : seules les nouvelles lignes sont décodées et ajoutées, au lieu de tout reconstruire à chaque octet reçu ; la sortie n'est plus une région annoncée aux lecteurs d'écran (chaque octet était lu).
- Servomoteur : plus de filtre CSS.

### Corrigé

- `tests/budget/mesure.sh` comptait des processus WebKit d'autres apps (Safari, Mail…) et manquait le firmware : seuls les processus dont Prebench est responsable sont comptés, et le firmware lancé par l'app (`run/firmware-<horodatage>`) est reconnu. Même correction pour le firmware dans `mesure.ps1`.

## [0.2.0] - 2026-11-14

**Aperçu technique** : fin du socle de la version 1 pour la Nucleo-F401RE. Après l'UART de la 0.1.3, le firmware pilote des PWM et lit des tensions sur l'ADC ; l'inspecteur montre la configuration et l'état de chaque broche, fil et composant.

### Ajouté

- **Inspecteur** en trois sections (Configuration, État, Net) pour une broche, un fil et un composant ; **Réinitialiser** garde la paillasse, **Couper l'alimentation** arrête le firmware et le relance depuis zéro à la remise sous tension.
- **PWM** des timers STM32 (HAL TIM) : fréquence et rapport cyclique, jamais les fronts ; une PWM à 20 kHz ne charge ni le canal ni le CPU. La LED suit le rapport cyclique (fondu de LD2).
- **ADC** (HAL ADC) : tension de la broche convertie selon la résolution et la tension de référence de la carte ; résolution, alignement, mode scan, conversion par interruption. Le DMA compile et avertit « non simulé ».
- Composants **Potentiomètre**, **Servomoteur** (angle selon la largeur d'impulsion, avertissement hors de 1 à 2 ms ou de 50 Hz), **Buzzer** (passif ou actif, fréquence affichée ; le son viendra avec l'audio) et **LED RGB** (anode ou cathode commune).
- Inspecteur : fréquence, rapport cyclique, tension et valeur ADC ; paramètres des composants modifiables en mode Édition.
- Exemples `tests/pwm-fade` (fondu de LD2 par TIM2_CH1, comme dans CubeMX) et `tests/pot-servo` (un potentiomètre commande un servo).
- Projet `tests/budget` et scripts `mesure.sh` (macOS) et `mesure.ps1` (Windows) pour mesurer la consommation de Prebench (CPU, RAM).

### Modifié

- Les fréquences d'horloge (`HAL_RCC_GetSysClockFreq`, `SystemCoreClock`, horloge des timers) sont déduites de la configuration passée à `HAL_RCC_OscConfig` et `HAL_RCC_ClockConfig`, comme sur la puce ; sans `SystemClock_Config`, le firmware tourne à 16 MHz (HSI), comme la puce au démarrage. Le temps (`HAL_Delay`, `HAL_GetTick`) n'en dépend pas.
- Composants : le budget de 5 ms par appel (et de 50 ms au chargement) se compte en temps de calcul, et non en durée écoulée : sur une machine chargée, un composant n'est plus arrêté à tort.
- Fichier de carte : tension de référence de l'ADC (`adc_vref`) ; composants : tension du net (`PinState.volts`). Ajouts compatibles des formats v1.

## [0.1.3] - 2026-11-13

**Aperçu technique** : la paillasse devient éditable (composants, fils, notes), Prebench vérifie le câblage et affiche les niveaux logiques, et l'UART du firmware arrive avec un moniteur série et un traceur.

### Ajouté

- Mode **Édition** (touche E) : ajouter, déplacer, renommer et supprimer des composants, tirer des fils entre les broches de la carte et des composants, poser des notes sur la paillasse. Annulation (⌘Z / Ctrl+Z). Tout est enregistré dans `prebench.toml` ; un fil modifié pendant la simulation s'applique aussitôt.
- Vue de la paillasse : déplacement, zoom, **Ajuster la vue** (⌘0 / Ctrl+0), taille réelle (⌘1 / Ctrl+1).
- Niveaux logiques des broches utilisées pendant la simulation (haut, bas, tiré, flottant, conflit), affichés sur les broches et dans l'inspecteur.
- Onglet **Avertissements** : vérification du câblage sans lancer la simulation (rôles des broches, bus, tension, 5 V sur une broche, courts-circuits, y compris un composant posé directement entre deux alimentations), puis avertissements de la simulation (entrée lue flottante, conflit entre deux sorties). Chaque phrase dit quoi corriger.
- UART du firmware simulée (HAL STM32 : émission et réception bloquantes, par interruption et DMA, `ReceiveToIdle`).
- Onglet **Série** : choix du port (USART du firmware ou sortie standard `printf`) et du débit du moniteur, horodatage, vue hexadécimale, envoi avec fin de ligne (LF, CR+LF, CR, aucune), **Exporter la sortie…** (fenêtre « Enregistrer sous », dossier du projet proposé), **Effacer**. Alerte si le débit du moniteur diffère de celui du firmware.
- Onglet **Traceur** : courbes des lignes `clé:valeur` (par exemple `temp:21.4,hum:55`), jusqu'à 8 courbes.

### Modifié

- Garde-fou CPU : la pause automatique compte le temps de calcul réellement consommé par le firmware (5 s à plus de 80 % d'un cœur), et non la durée écoulée : une machine chargée ne la déclenche plus trop tôt ni trop tard.

### Corrigé

- Deux simulations successives ne peuvent plus ouvrir le même canal de communication avec le firmware.
- macOS : quitter Prebench pendant une pause ne laisse plus le processus du firmware suspendu en arrière-plan.

### Limites connues

- macOS : en cas de plantage de Prebench pendant une pause, le processus du firmware reste suspendu ; le terminer depuis le Moniteur d'activité (« firmware »). Windows n'est pas concerné (Job Object).
- Le traceur lit le port affiché dans l'onglet Série.

## [0.1.2] - 2026-11-13

**Aperçu technique** : Prebench suit maintenant le dossier du projet (recompilation et rechargement à chaque sauvegarde), se débogue dans VS Code, protège l'ordinateur contre un firmware qui s'emballe, et adopte sa nouvelle interface.

### Ajouté

- Nouvelle interface : paillasse sur tapis antistatique, carte dessinée à plat, inspecteur et panneau du bas repliables, barre d'état ; thème clair, sombre ou celui du système (bouton de la barre d'outils, choix mémorisé).
- Recompilation à chaque sauvegarde dans ton éditeur : si la simulation tourne, le firmware est rechargé sans toucher à la paillasse ; en cas d'erreur, l'ancien firmware continue de tourner.
- Onglet **Build** : erreurs et avertissements du compilateur, avec `fichier:ligne` cliquable qui ouvre VS Code à la bonne ligne (sinon l'application associée au fichier). État du build dans la barre d'état.
- Composants du projet (`components/<id>/`) : une vue modifiée est rechargée aussitôt ; un modèle ou `prebench.toml` modifié relance la simulation.
- **Déboguer** : le firmware se débogue dans VS Code avec des points d'arrêt (CodeLLDB sur macOS ; extension C/C++ et gdb sous Windows). Prebench ajoute la configuration à `.vscode/launch.json` et signale l'extension ou le débogueur manquant. Sur un point d'arrêt, la paillasse se fige (« Arrêté sur un point d'arrêt ») et le temps du firmware repart sans saut.
- IntelliSense de VS Code : le build produit `compile_commands.json`, inscrit dans `.vscode/settings.json` du projet ; les en-têtes simulés (`stm32f4xx_hal.h`…) sont trouvés.
- **Mettre en pause** et **Reprendre** (touche Espace), **Réinitialiser** (touche R), **Arrêter**.
- Indicateur CPU et RAM du firmware dans la barre d'état.
- Garde-fous : un firmware qui occupe plus de 80 % d'un cœur pendant 5 s est mis en pause (bannière « Reprendre ») ; au-delà de 256 Mo de mémoire, il est arrêté avec une explication.

### Corrigé

- Le rapport de `--verifier-installation` n'est plus affiché deux fois sous macOS.
- Windows : le firmware est rattaché à un Job Object : il s'arrête avec l'app, même en cas de fermeture brutale.

### Limites connues

- macOS n'applique pas de plafond mémoire système : la limite de 256 Mo est surveillée par Prebench (4 fois par seconde au-delà de 128 Mo) ; une allocation très rapide peut la dépasser brièvement avant l'arrêt.
- La carte n'a pas encore de connecteurs : le câblage de composants externes arrive dans une prochaine version.

## [0.1.1] - 2026-11-11

**Aperçu technique** : corrige la compilation avec l'app installée sous Windows et facilite la création d'un premier projet.

### Corrigé

- Windows : échec du build (« target pattern contains no '%' ») avec l'app installée dans `C:\Program Files` : les chemins sont maintenant transmis à CMake sans le préfixe `\\?\`.
- Le firmware n'est plus compilé dans le dossier du projet ni dans celui de l'installation (en lecture seule sous Windows), mais dans le cache de Prebench. Si la configuration CMake échoue, ce build est supprimé et le suivant repart de zéro ; une erreur de compilation le garde.
- Windows : « CMake introuvable » quand CMake n'est pas dans le PATH (cas par défaut de son installateur). Prebench cherche CMake, le compilateur et make dans le PATH, puis aux emplacements standard (`C:\Program Files\CMake\bin`, `C:\msys64\ucrt64\bin` ; sous macOS `/opt/homebrew/bin`, `/usr/local/bin`, `/Applications/CMake.app`).

### Ajouté

- **Nouveau projet…** : copie d'un modèle (blinky STM32 HAL) dans `Documents/Prebench`, pour le modifier sans toucher au modèle installé. Au premier lancement, **Créer mon premier projet** prépare la copie du blinky.
- Vérification des prérequis au démarrage (CMake, compilateur) : un bandeau indique ce qui manque et la commande pour l'installer.
- Les chemins de CMake et du compilateur utilisés sont écrits dans le Journal à chaque démarrage de la simulation.

## [0.1.0] - 2026-11-10

**Aperçu technique** : première version publiée, pour essayer le principe de Prebench. Elle simule une seule carte, la Nucleo-F401RE, et seulement ses GPIO.

### Ajouté

- Simulation d'un projet STM32 HAL sur Nucleo-F401RE, sans modifier le code : un projet généré par STM32CubeMX se compile tel quel pour l'ordinateur et s'exécute dans Prebench.
- GPIO, interruptions EXTI, `HAL_Delay` et `HAL_GetTick` simulés. La LED LD2 et le bouton B1 de la carte sont intégrés à la paillasse.
- UART, TIM, ADC, I2C, SPI et DMA : le code compile, et Prebench affiche « non simulé » dans l'onglet ⚠ ; leur simulation arrive dans les versions suivantes.
- Composants LED, bouton poussoir et écho (mesure de latence).
- Projet de départ « blinky STM32 HAL » : LD2 clignote, B1 change la vitesse.
- Onglet Journal (sorties du firmware) et onglet ⚠ (avertissements).
- Firmware exécuté en priorité basse, pour ne pas ralentir l'ordinateur ; la priorité effective est affichée dans le Journal.
- Mises à jour automatiques, vérifiées par signature.

### Limites connues

- La boucle principale vide d'un projet CubeMX (`while (1) {}`) occupe un cœur à 100 % ; le plafond CPU avec pause automatique arrive dans une version suivante.
- Pas encore de câblage dans l'interface : les composants se déclarent dans `prebench.toml`.
- Le projet est compilé quand on clique **▶ Démarrer** ; la recompilation à chaque sauvegarde arrive dans une version suivante.
- L'app n'est pas signée : voir la procédure de premier lancement dans le README (macOS et Windows).
