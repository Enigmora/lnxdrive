# Estrategia de Internacionalizacion (i18n)

> **Ubicación:** `08-Distribucion/04-internacionalizacion.md`
> **Relacionado:** [Estructura de Repositorios](01-estructura-repositorios.md), [UI GNOME](../04-Componentes/02-ui-gnome.md)

---

## Parte XVIII: Estrategia de Internacionalizacion (i18n)

> **Objetivo:** Definir que componentes del proyecto requieren internacionalizacion, que idiomas se soportaran, y como se gestionara el proceso de traduccion.

### 18.1 Filosofia: Pragmatismo en i18n

No todos los componentes de LNXDrive estan dirigidos a usuarios finales. La estrategia de internacionalizacion sigue un principio simple:

- **Componentes tecnicos**: Solo ingles (logs, CLI, mensajes de error internos)
- **Interfaces de usuario**: Internacionalizadas (UIs graficas, notificaciones, metadatos de aplicacion)

### 18.2 Analisis por Componente

| Componente | Audiencia | i18n | Justificacion |
|------------|-----------|------|---------------|
| **lnxdrive-core** | Interno | No | Libreria sin mensajes directos a usuario |
| **lnxdrive-daemon** | Sysadmin/Logs | No | Logs tecnicos para soporte, ingles estandar |
| **lnxdrive-fuse** | Interno | No | Errores propagados al daemon |
| **lnxdrive-cli** | Tecnico | No | Usuarios tecnicos, scripting, consistencia Unix |
| **lnxdrive-gnome** | Usuario final | Si | Interfaz grafica completa |
| **lnxdrive-gtk3** | Usuario final | Si | Interfaz grafica completa |
| **lnxdrive-plasma** | Usuario final | Si | Interfaz grafica completa |
| **lnxdrive-cosmic** | Usuario final | Si | Interfaz grafica completa |
| **Notificaciones D-Bus** | Usuario final | Si | Visibles en escritorio |
| **Archivos .desktop** | Usuario final | Si | Menus del sistema |
| **AppStream/Metainfo** | Usuario final | Si | Tiendas de aplicaciones |

### 18.3 Idiomas Soportados

#### Fase Inicial

| Idioma | Codigo | Estado |
|--------|--------|--------|
| Ingles | `en` | Base (fuente) |
| Espanol | `es` | Traduccion primaria |

#### Expansion Futura

Idiomas adicionales se agregaran segun demanda de la comunidad, evaluando:
- Numero de solicitudes en GitHub Issues
- Disponibilidad de revisores nativos para QA
- Relevancia en distribuciones Linux objetivo

### 18.4 Formatos de Traduccion por Toolkit

Cada toolkit tiene su formato estandar de internacionalizacion:

| Repositorio | Toolkit | Formato | Herramienta de Extraccion |
|-------------|---------|---------|---------------------------|
| lnxdrive-gnome | GTK4 + libadwaita | gettext (.po) | `xgettext`, `msgfmt` |
| lnxdrive-gtk3 | GTK3 | gettext (.po) | `xgettext`, `msgfmt` |
| lnxdrive-plasma | Qt6 | Qt Linguist (.ts/.qm) | `lupdate`, `lrelease` |
| lnxdrive-cosmic | iced + libcosmic | Fluent (.ftl) | Manual / `cargo-i18n` |

### 18.5 Estructura de Archivos por Repositorio

#### lnxdrive-gnome (gettext)

```
lnxdrive-gnome/
├── po/
│   ├── POTFILES.in              # Lista de archivos fuente a escanear
│   ├── LINGUAS                  # Idiomas disponibles: en es
│   ├── lnxdrive-gnome.pot       # Template extraido (strings en ingles)
│   └── es.po                    # Traduccion al espanol
├── data/
│   ├── org.enigmora.LNXDrive.desktop.in
│   └── org.enigmora.LNXDrive.metainfo.xml.in
└── src/
    └── *.rs                     # Codigo con gettext!() macros
```

**Ejemplo de codigo Rust con gettext:**

```rust
use gettextrs::gettext;

fn show_sync_status(files: u32) {
    // String simple
    let title = gettext("Synchronization Complete");

    // String con formato
    let message = gettextrs::ngettext(
        "1 file synchronized",
        "{} files synchronized",
        files as u32
    ).replace("{}", &files.to_string());

    show_notification(&title, &message);
}
```

**Ejemplo de archivo es.po:**

```po
# Spanish translation for lnxdrive-gnome
# Copyright (C) 2026 Enigmora
# This file is distributed under the same license as the lnxdrive-gnome package.
#
msgid ""
msgstr ""
"Project-Id-Version: lnxdrive-gnome 1.0\n"
"Language: es\n"
"MIME-Version: 1.0\n"
"Content-Type: text/plain; charset=UTF-8\n"
"Content-Transfer-Encoding: 8bit\n"
"Plural-Forms: nplurals=2; plural=(n != 1);\n"

#: src/window.rs:45
msgid "Synchronization Complete"
msgstr "Sincronizacion Completada"

#: src/window.rs:48
msgid "1 file synchronized"
msgid_plural "{} files synchronized"
msgstr[0] "1 archivo sincronizado"
msgstr[1] "{} archivos sincronizados"

#: src/preferences.rs:23
msgid "Settings"
msgstr "Configuracion"

#: src/preferences.rs:67
msgid "Sync Folder"
msgstr "Carpeta de Sincronizacion"
```

#### lnxdrive-gtk3 (gettext)

```
lnxdrive-gtk3/
├── po/
│   ├── POTFILES.in
│   ├── LINGUAS                  # en es
│   ├── lnxdrive-gtk3.pot
│   └── es.po
├── data/
│   └── lnxdrive-gtk3.desktop.in
└── src/
    └── *.rs
```

#### lnxdrive-plasma (Qt Linguist)

```
lnxdrive-plasma/
├── translations/
│   ├── lnxdrive-plasma.ts       # Template (generado por lupdate)
│   ├── lnxdrive-plasma_es.ts    # Traduccion espanol
│   └── lnxdrive-plasma_es.qm    # Compilado (binario)
├── src/
│   └── *.cpp                    # Codigo con tr() y QT_TR_NOOP()
└── qml/
    └── *.qml                    # QML con qsTr()
```

**Ejemplo de codigo C++:**

```cpp
#include <KLocalizedString>

void SyncWidget::showStatus(int files) {
    // i18n de KDE Frameworks
    QString title = i18n("Synchronization Complete");
    QString message = i18np(
        "1 file synchronized",
        "%1 files synchronized",
        files
    );

    showNotification(title, message);
}
```

**Ejemplo de QML:**

```qml
import QtQuick 2.15
import org.kde.kirigami 2.20 as Kirigami

Kirigami.Page {
    title: qsTr("Settings")

    Kirigami.FormLayout {
        TextField {
            Kirigami.FormData.label: qsTr("Sync Folder:")
        }

        Button {
            text: qsTr("Sync Now")
        }
    }
}
```

**Ejemplo de archivo .ts:**

```xml
<?xml version="1.0" encoding="utf-8"?>
<!DOCTYPE TS>
<TS version="2.1" language="es">
<context>
    <name>SyncWidget</name>
    <message>
        <location filename="../src/syncwidget.cpp" line="45"/>
        <source>Synchronization Complete</source>
        <translation>Sincronizacion Completada</translation>
    </message>
    <message numerus="yes">
        <location filename="../src/syncwidget.cpp" line="48"/>
        <source>%n file(s) synchronized</source>
        <translation>
            <numerusform>%n archivo sincronizado</numerusform>
            <numerusform>%n archivos sincronizados</numerusform>
        </translation>
    </message>
</context>
<context>
    <name>main</name>
    <message>
        <location filename="../qml/main.qml" line="12"/>
        <source>Settings</source>
        <translation>Configuracion</translation>
    </message>
</context>
</TS>
```

#### lnxdrive-cosmic (Fluent)

```
lnxdrive-cosmic/
├── i18n/
│   ├── en/
│   │   └── lnxdrive-cosmic.ftl  # Ingles (fuente)
│   └── es/
│       └── lnxdrive-cosmic.ftl  # Espanol
└── src/
    └── *.rs                     # Codigo con fl!() macro
```

**Ejemplo de archivo Fluent (en/lnxdrive-cosmic.ftl):**

```fluent
# General
app-name = LNXDrive
settings = Settings

# Sync status
sync-complete = Synchronization Complete
sync-files =
    { $count ->
        [one] 1 file synchronized
       *[other] { $count } files synchronized
    }

# Sync actions
sync-now = Sync Now
sync-pause = Pause
sync-resume = Resume

# Errors
error-connection = Connection error: { $message }
error-auth = Authentication failed. Please log in again.

# Preferences
pref-sync-folder = Sync Folder
pref-startup = Start on login
pref-notifications = Show notifications
```

**Ejemplo de archivo Fluent (es/lnxdrive-cosmic.ftl):**

```fluent
# General
app-name = LNXDrive
settings = Configuracion

# Sync status
sync-complete = Sincronizacion Completada
sync-files =
    { $count ->
        [one] 1 archivo sincronizado
       *[other] { $count } archivos sincronizados
    }

# Sync actions
sync-now = Sincronizar Ahora
sync-pause = Pausar
sync-resume = Reanudar

# Errors
error-connection = Error de conexion: { $message }
error-auth = Autenticacion fallida. Por favor, inicie sesion nuevamente.

# Preferences
pref-sync-folder = Carpeta de Sincronizacion
pref-startup = Iniciar al arrancar
pref-notifications = Mostrar notificaciones
```

**Ejemplo de codigo Rust con Fluent:**

```rust
use cosmic::iced::widget::text;
use i18n_embed_fl::fl;

fn sync_status_view(files: u32) -> Element<Message> {
    let title = fl!("sync-complete");
    let message = fl!("sync-files", count = files);

    column![
        text(title).size(18),
        text(message).size(14),
    ]
    .into()
}
```

### 18.6 Flujo de Traduccion

El proceso de traduccion esta automatizado mediante un agente de IA:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                       FLUJO DE TRADUCCION                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │ 1. DESARROLLO                                                        │   │
│  │    Desarrollador agrega/modifica strings en codigo fuente            │   │
│  └─────────────────────────────────┬───────────────────────────────────┘   │
│                                    │                                        │
│                                    ▼                                        │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │ 2. EXTRACCION                                                        │   │
│  │    Ejecutar herramienta de extraccion segun toolkit:                 │   │
│  │    * gettext: xgettext -> .pot                                       │   │
│  │    * Qt: lupdate -> .ts                                              │   │
│  │    * Fluent: manual o cargo-i18n                                     │   │
│  └─────────────────────────────────┬───────────────────────────────────┘   │
│                                    │                                        │
│                                    ▼                                        │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │ 3. TRADUCCION (Agente IA)                                            │   │
│  │    Agente de IA procesa archivos de template y genera traducciones   │   │
│  │    para cada idioma configurado                                      │   │
│  └─────────────────────────────────┬───────────────────────────────────┘   │
│                                    │                                        │
│                                    ▼                                        │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │ 4. COMMIT                                                            │   │
│  │    Archivos de traduccion actualizados se commitean al repositorio   │   │
│  └─────────────────────────────────┬───────────────────────────────────┘   │
│                                    │                                        │
│                                    ▼                                        │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │ 5. COMPILACION (CI/CD)                                               │   │
│  │    * gettext: msgfmt -> .mo                                          │   │
│  │    * Qt: lrelease -> .qm                                             │   │
│  │    * Fluent: incluido en build                                       │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 18.7 Gestion de Errores de Traduccion

Los errores de traduccion reportados por usuarios se gestionan mediante GitHub Issues:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                  FLUJO DE CORRECCION DE TRADUCCIONES                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  1. Usuario detecta error de traduccion                                     │
│                    │                                                        │
│                    ▼                                                        │
│  2. Abre Issue en GitHub del repositorio correspondiente                    │
│     Label: "translation" + "es" (o idioma correspondiente)                  │
│                    │                                                        │
│                    ▼                                                        │
│  3. Issue incluye:                                                          │
│     * Texto actual (incorrecto)                                             │
│     * Texto sugerido (correccion)                                           │
│     * Ubicacion en la UI (screenshot opcional)                              │
│                    │                                                        │
│                    ▼                                                        │
│  4. Maintainer o agente IA corrige el archivo de traduccion                 │
│                    │                                                        │
│                    ▼                                                        │
│  5. PR con correccion -> Review -> Merge                                    │
│                    │                                                        │
│                    ▼                                                        │
│  6. Incluido en proximo release                                             │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Template de Issue para errores de traduccion:**

```markdown
---
name: Translation Error
about: Report an incorrect or missing translation
labels: translation
---

**Language:** es (Spanish)

**Location in UI:**
<!-- Where does this text appear? Include screenshot if possible -->

**Current text:**
<!-- The current incorrect translation -->

**Suggested text:**
<!-- Your suggested correction -->

**Additional context:**
<!-- Any additional context about why the current translation is incorrect -->
```

### 18.8 Archivos .desktop y Metainfo

Los archivos de metadatos de aplicacion tambien requieren traduccion:

#### Archivo .desktop

```ini
# data/org.enigmora.LNXDrive.desktop.in
[Desktop Entry]
Name=LNXDrive
Name[es]=LNXDrive
GenericName=Cloud Storage Client
GenericName[es]=Cliente de Almacenamiento en la Nube
Comment=Sync your files with cloud storage providers
Comment[es]=Sincroniza tus archivos con proveedores de almacenamiento en la nube
Exec=lnxdrive-gnome
Icon=org.enigmora.LNXDrive
Terminal=false
Type=Application
Categories=Network;FileTransfer;
Keywords=cloud;sync;onedrive;gdrive;dropbox;storage;
Keywords[es]=nube;sincronizar;onedrive;gdrive;dropbox;almacenamiento;
```

#### Archivo AppStream Metainfo

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!-- data/org.enigmora.LNXDrive.metainfo.xml.in -->
<component type="desktop-application">
  <id>org.enigmora.LNXDrive</id>
  <name>LNXDrive</name>
  <name xml:lang="es">LNXDrive</name>
  <summary>Cloud storage client for Linux desktops</summary>
  <summary xml:lang="es">Cliente de almacenamiento en la nube para escritorios Linux</summary>
  <description>
    <p>
      LNXDrive is a native cloud storage client for Linux that integrates
      seamlessly with your desktop environment. Supports OneDrive, Google Drive, Dropbox, and more.
    </p>
    <p xml:lang="es">
      LNXDrive es un cliente nativo de almacenamiento en la nube para Linux que se integra
      perfectamente con tu entorno de escritorio. Soporta OneDrive, Google Drive, Dropbox, y más.
    </p>
    <p>Features include:</p>
    <p xml:lang="es">Caracteristicas incluidas:</p>
    <ul>
      <li>Files on Demand - Access files without downloading</li>
      <li xml:lang="es">Archivos Bajo Demanda - Accede a archivos sin descargarlos</li>
      <li>Automatic synchronization</li>
      <li xml:lang="es">Sincronizacion automatica</li>
      <li>Native desktop integration</li>
      <li xml:lang="es">Integracion nativa con el escritorio</li>
    </ul>
  </description>

  <launchable type="desktop-id">org.enigmora.LNXDrive.desktop</launchable>
  <url type="homepage">https://github.com/enigmora/lnxdrive</url>
  <url type="bugtracker">https://github.com/enigmora/lnxdrive/issues</url>

  <developer id="org.enigmora">
    <name>Enigmora</name>
  </developer>

  <content_rating type="oars-1.1" />
  <releases>
    <release version="1.0.0" date="2026-XX-XX">
      <description>
        <p>Initial release</p>
        <p xml:lang="es">Version inicial</p>
      </description>
    </release>
  </releases>
</component>
```

### 18.9 Integracion con Build System

#### Meson (GNOME/GTK)

```meson
# lnxdrive-gnome/meson.build

i18n = import('i18n')

# Configurar gettext
i18n.gettext(
  meson.project_name(),
  preset: 'glib',
)

# Procesar .desktop
i18n.merge_file(
  input: 'data/org.enigmora.LNXDrive.desktop.in',
  output: 'org.enigmora.LNXDrive.desktop',
  type: 'desktop',
  po_dir: 'po',
  install: true,
  install_dir: get_option('datadir') / 'applications',
)

# Procesar metainfo
i18n.merge_file(
  input: 'data/org.enigmora.LNXDrive.metainfo.xml.in',
  output: 'org.enigmora.LNXDrive.metainfo.xml',
  type: 'xml',
  po_dir: 'po',
  install: true,
  install_dir: get_option('datadir') / 'metainfo',
)
```

#### CMake (KDE/Qt)

```cmake
# lnxdrive-plasma/CMakeLists.txt

find_package(ECM REQUIRED NO_MODULE)
find_package(KF6I18n REQUIRED)

# Generar archivos de traduccion
ki18n_install(po)

# Compilar traducciones Qt
qt_add_translations(lnxdrive-plasma
    TS_FILES
        translations/lnxdrive-plasma_es.ts
    QM_FILES_OUTPUT_VARIABLE qm_files
)
install(FILES ${qm_files} DESTINATION ${KDE_INSTALL_LOCALEDIR})
```

### 18.10 Resumen de Decisiones

| Aspecto | Decision |
|---------|----------|
| **Componentes con i18n** | Solo UIs graficas (GNOME, GTK3, Plasma, Cosmic) |
| **Componentes sin i18n** | Core, Daemon, FUSE, CLI |
| **Idiomas iniciales** | Ingles (base) + Espanol |
| **Metodo de traduccion** | Agente de IA |
| **Plataforma de traduccion** | Ninguna (archivos en repositorio) |
| **Gestion de errores** | GitHub Issues con label "translation" |
| **Formatos** | gettext (.po), Qt Linguist (.ts), Fluent (.ftl) |

---

## Ver tambien

- [01-estructura-repositorios.md](01-estructura-repositorios.md) - Estructura de repositorios
- [02-comunicacion-dbus.md](02-comunicacion-dbus.md) - Comunicacion D-Bus
- [03-gobernanza-proyecto.md](03-gobernanza-proyecto.md) - Gobernanza del proyecto
