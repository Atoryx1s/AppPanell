# v1.0:
- Program release with bugs
- Add logic program and UI/UX

# v1.1.0
- Fix more bugs
- Add system tray
- Create a new release
- Add Changelog
- Create Readme

# v1.1.1
- Add author for installing process

# v1.1.2
- Fixing bug hide the application when clicked outside its boundaries

# v.1.2.1
- Add auto update method application

# v1.2.2
- Add GitHub Actions

# v1.2.3
- Fix update app method

# v1.2.4
- Fixed an issue with the highlighting of shortcuts in the middle of the panel 

# v1.2.5
- Fix update json to release and add button notification

# Internal Infrastructure Update (v1.2.6 - v1.2.24)

* *`Fixed`*: Issues with automated application signing in GitHub Actions.

* *`Added`*: Automatic generation of `latest.json` for the built-in updater.

* *`Note`*: These versions were used for internal CI/CD testing.

# v1.2.25
- Fixed bug and Upgrade app

# v1.2.26
- Fix UI update button

# Technical Changes (v1.2.26 – v1.2.35)

* `Security`: Implemented a digital signature verification system (Ed25519) for secure loading of executable files.

* `Stability`: Fixed installer extraction errors and ensured proper integration with GitHub Releases.

* `Optimization`: Improved version checking logic when 
launching the application.

# v1.2.36 — April 6, 2026
* `New`: Automatic Updates
* `Update System`: Full support for remote updates has been added. You no longer need to manually download new versions from GitHub.

* `Update Indicator`: A ↓ icon will appear in the top bar when a new version is available.

* `One-Click Installation`: Added an update confirmation dialog. The program will download, verify the signature, and reinstall itself automatically.

# v1.2.37
* `Added`: real-time update checking

# v1.2.38 — April 7, 2026

* `Changed`: Transferred project ownership to [Atoryxic Labs].

* `Updated`: License file and contact information in README.md.

# v.1.2.39 — April 15, 2026

- Added Linux (x86_64) platform support
- Linux builds are provided as portable AppImage
- Cross-platform updater support (Windows + Linux)
- Updated release infrastructure to generate unified `latest.json`

# v1.2.40 - May 6, 2026

- Fix bug single instance