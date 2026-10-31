# Blender film v2: scripted bpy scene rendered on Kaggle

Plan + progress + finding. Film v2 of the Streams Reader story (room at night, headphones on, park at golden hour, end
card) rebuilt in Blender from a shot JSON (`media/blender/`), rendered headless on a private Kaggle T4 kernel, mixed
locally with the existing voice-over and music. Decision behind it: D33 in 05-decisions.md.

## 2026-10-31

### Plan and acceptance checks (status filled at the end)
1. `media/blender/build_scene.py` builds the whole film from `media/blender/shots/film.json` in pure bpy: armature
   with IK arms (hands on the phone/book) and IK legs (feet planted), procedural walk with planted feet, blink and
   mouth, headphone cable curve with sway, bevelled voxel meshes with procedural texture, room and park sets, PBR
   lights, DOF with rack focus, motion blur, AgX + grade. Check: kernel log shows the build with no errors. (open)
2. `media/blender/kaggle/run.py` pushes a private GPU kernel, waits, downloads outputs. Check: one preview still per
   shot downloaded and read. (open)
3. Renderer choice measured: Cycles OptiX vs EEVEE on the T4, seconds per frame at 1080p. (open)
4. Full 41 s render at 1920x1080 30 fps; stills every 0.5 s read and judged. (open)
5. `media/blender/post.py`: titles, end card, VO + music mix, WebM + MP4 <= 10 MB + poster. (open)

## Open items
- See the plan checks above.
