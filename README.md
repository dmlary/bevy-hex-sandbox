# bevy-hex-sandbox
Example implementation of a 3d hexagon tile-based map editor using bevy v0.10.

**This is not a maintained project.**

Instead, this is a series of example implementations of how a series of
capabilities could be implemented in bevy.  There are no guarantees that
any of the implementations are idiomatic.

Capabilities/Functionality:
* Maintainable/scaleable egui interface
    * adapted from https://github.com/bevyengine/bevy/discussions/5522
    * widgets aren't required to implement `SystemParam`
    * implementation: src/ui/widget.rs
    * complex usage: src/bin/editor_ui/panel.rs
* Reorganizable egui tile picker
    * multi-select, drag & drop to sort
    * implementation `editor_ui::panel::TilePicker`
* Render images of GLTF models
    * use a second camera & RenderLayers to render each GLTF scene into an image
    * implementation: src/thumbnail_render.rs
* native file pickers (save & load) via IoTask & rfd crate
    * src/file_picker.rs
* IoTask-based save & load (not using Assets)
    * When bevy's Asset infrastructure is too heavy-weight for loading config
      data, we can use IoTask to load & return the results
    * implementation `tileset::tileset_importer` & `tileset::tileset_exporter`
    * more complicated example in src/persistence.rs
* Serialize (not reflection) based save/load
    * When you need to save a framework agnostic format (example: map file)
      reflection-based formats expose too many internals
    * Reflection-based save also causes noise in diffs due to Entity values
      changing between each run
    * implementation: `src::persistence::MapFormat`
    * not fully diff-stable at this time (tiles not sorted)
    * format needs improvements for diffs to be fully human readable
* version-aware save/load
    * limited support, recognizes incorrect version
    * can be expanded to generate intermediate representation based on what the
      version provides
    * see Tileset::Serialize/Deserialize

## Assets
Assets included in this project were created by Kenney, and available at
https://kenney.nl/assets/hexagon-kit

## Usage
* File -> New Map
* In the Tileset panel, click the "..." button and select "Import Tileset"
    * Note: bug here, may have to click it multiple times
* Select "kenney.tileset.ron"
* click on tile then click on map

## Controls
* Q/E: Rotate currently selected tile
* `[` / `]`: Rotate camera
* Scroll wheel: zoom in/out
* Space + mouse move: pan camera
