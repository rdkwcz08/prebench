# Journal des modifications

Les évolutions de Prebench qui comptent pour les utilisateurs sont consignées ici.

Format inspiré de [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/). Versions [SemVer](https://semver.org/lang/fr/) en `0.x` jusqu'à la stabilité. `release.sh` exige une section `## [X.Y.Z]` avant de publier une version.

## [Non publié]

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
