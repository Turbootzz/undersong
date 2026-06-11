#!/usr/bin/env python3
# The map builder behind every committed map.ron (P3–P8). Maps are the
# artifact of record; this script regenerates them deterministically.
# Run from the repo root: python3 scripts/mapgen.py
import os

class M:
    def __init__(s, mid, w, h, ground_fill=1):
        s.id, s.w, s.h = mid, w, h
        s.ground = [[ground_fill]*w for _ in range(h)]
        s.decor = [[0]*w for _ in range(h)]
        s.overhang = [[0]*w for _ in range(h)]
        s.coll = [[0]*w for _ in range(h)]
        s.patch = [[0]*w for _ in range(h)]
        s.triggers, s.npcs = [], []
        s.encounters = None
        s.night_encounters = None
        s.music = None
        s.weather = None
        s.dark = False
        s.indoor = False
        s.obstacles = []
        # border walls
        for x in range(w):
            s.coll[0][x] = s.coll[h-1][x] = 1
            s.ground[0][x] = s.ground[h-1][x] = 3
        for y in range(h):
            s.coll[y][0] = s.coll[y][w-1] = 1
            s.ground[y][0] = s.ground[y][w-1] = 3
    def rect(s, x0, y0, x1, y1, layer, v, solid=None):
        for y in range(y0, y1+1):
            for x in range(x0, x1+1):
                getattr(s, layer)[y][x] = v
                if solid is not None: s.coll[y][x] = solid
    def door(s, x, y, target, to, facing):
        s.coll[y][x] = 0
        s.ground[y][x] = 2
        s.triggers.append((x, y, f'Warp(map: "{target}", to: ({to[0]}, {to[1]}), facing: {facing})', None))
    def script_trigger(s, x, y, path, once=None):
        once_txt = f'Some("{once}")' if once else None
        s.triggers.append((x, y, f'Script(path: "{path}")', once))
    def npc(s, nid, x, y, facing, sprite, script=None, wander=None, sight=0):
        s.npcs.append((nid, x, y, facing, sprite, script, wander, sight))
    def emit(s, root):
        d = os.path.join(root, s.id)
        os.makedirs(os.path.join(d, 'scripts'), exist_ok=True)
        def layer(rows, name, required=True):
            flat = []
            for y in range(s.h):  # y0 first (bottom-first storage)
                flat.append(", ".join(str(v) for v in rows[y]))
            body = ",\n        ".join(flat)
            return f"    {name}: [\n        {body},\n    ],\n"
        parts = [f'// {s.id} — generated slice map (P3). Bottom row first; y up.\n',
                 'MapDef(\n',
                 f'    id: "{s.id}",\n    name_key: "map.{s.id}",\n',
                 f'    width: {s.w},\n    height: {s.h},\n',
                 layer(s.ground, 'ground'),
                 layer(s.decor, 'decor'),
                 layer(s.overhang, 'overhang'),
                 layer(s.coll, 'collision'),
                 layer(s.patch, 'patches')]
        trig = []
        for (x, y, kind, once) in s.triggers:
            extra = f', once_flag: Some("{once}")' if once else ''
            trig.append(f'        (at: ({x}, {y}), kind: {kind}{extra}),')
        parts.append("    triggers: [\n" + "\n".join(trig) + "\n    ],\n" if trig else "    triggers: [],\n")
        np = []
        for (nid, x, y, facing, sprite, script, wander, sight) in s.npcs:
            sc = f'Some("{script}")' if script else 'None'
            behavior = f'Wander(radius: {wander})' if wander else 'Static'
            np.append(f'        (id: "{nid}", at: ({x}, {y}), facing: {facing}, sprite: "{sprite}", script: {sc}, behavior: {behavior}, sight_range: {sight}),')
        parts.append("    npcs: [\n" + "\n".join(np) + "\n    ],\n" if np else "    npcs: [],\n")
        if s.encounters:
            rate, slots = s.encounters
            rows = ", ".join(f'("{sp}", {lo}, {hi}, {w})' for (sp, lo, hi, w) in slots)
            parts.append(f"    encounters: Some((\n        patch_rate_pct: {rate},\n        slots: [{rows}],\n    )),\n")
        if s.night_encounters:
            rate, slots = s.night_encounters
            rows = ", ".join(f'("{sp}", {lo}, {hi}, {w})' for (sp, lo, hi, w) in slots)
            parts.append(f"    night_encounters: Some((\n        patch_rate_pct: {rate},\n        slots: [{rows}],\n    )),\n")
        if s.obstacles:
            rows = "\n".join(f'        (at: ({x}, {y}), kind: {k}),' for (x, y, k) in s.obstacles)
            parts.append("    obstacles: [\n" + rows + "\n    ],\n")
        if getattr(s, 'weather', None):
            parts.append(f"    weather: Some({s.weather}),\n")
        if getattr(s, 'dark', False):
            parts.append("    dark: true,\n")
        if getattr(s, 'indoor', False):
            parts.append("    indoor: true,\n")
        parts.append(f"    music: {('Some(\"' + s.music + '\")') if getattr(s, 'music', None) else 'None'},\n)\n")
        open(os.path.join(d, 'map.ron'), 'w').write("".join(parts))

root = 'content/regions/cantorel/maps'

# ---------- pausa_village 20x14 ----------
p = M('pausa_village', 20, 14)
p.rect(4, 4, 15, 9, 'ground', 2)                      # village square (path)
p.rect(5, 9, 8, 11, 'ground', 4, solid=1)             # Reed's lab block
p.door(6, 9, 'pausa_lab', (4, 1), 'Up')               # lab door (south face)
p.rect(12, 9, 14, 11, 'ground', 4, solid=1)           # player home
p.script_trigger(13, 8, 'home_rest.script.ron')       # mom heals at the doorstep
p.rect(3, 2, 4, 2, 'decor', 5)
p.music = 'town_pausa'
p.npc('villager_mez', 10, 6, 'Down', 'npc.villager', None, wander=2)
p.npc('signpost_keeper', 15, 5, 'Left', 'npc.villager2', 'pausa_sign.script.ron')
# north exit to route 1 (x9,10)
for x in (9, 10):
    p.door(x, 13, 'route_1', (6 + (x-9), 1), 'Up')
# west exit to route 2 (act 1)
for y in (6, 7):
    p.door(15, y, 'route_2', (1, y), 'Right')
p.emit(root)

# ---------- pausa_lab 9x7 ----------
lab = M('pausa_lab', 9, 7, ground_fill=4)
lab.indoor = True
lab.rect(1, 5, 7, 5, 'ground', 4, solid=1)            # back bench row
lab.coll[5][4] = 0                                     # gap behind Reed
lab.music = 'town_pausa'
lab.npc('reed', 4, 4, 'Down', 'npc.reed', 'reed_intro.script.ron')
lab.script_trigger(4, 2, 'lab_enter.script.ron', once='story.intro.lab_seen')
lab.script_trigger(3, 2, 'theft_scene.script.ron')
lab.door(4, 0, 'pausa_village', (6, 8), 'Down')
lab.emit(root)

# ---------- route_1 14x30 ----------
r = M('route_1', 14, 30)
r.rect(5, 1, 8, 28, 'ground', 2)                      # main path
for (y0, y1) in [(5, 9), (14, 18), (22, 25)]:         # patch fields west+east
    r.rect(2, y0, 4, y1, 'patch', 1)
    r.rect(2, y0, 4, y1, 'ground', 1)
    r.rect(9, y0+1, 11, y1+1, 'patch', 1)
r.encounters = (12, [
    ("tremole", 2, 4, 20), ("pipling", 2, 4, 20), ("burrbass", 3, 5, 10),
    ("buzzoon", 3, 5, 10), ("solfawn", 3, 4, 10), ("zapresto", 3, 5, 10),
    ("glissicle", 4, 5, 5), ("timpanite", 4, 5, 5), ("dirgeist", 4, 6, 4),
    ("carilloy", 4, 6, 4), ("rattacca", 6, 7, 1), ("soarprano", 6, 7, 1),
])
# four trainers along the path with LoS
r.night_encounters = (12, [
    ("dirgeist", 3, 6, 20), ("glissicle", 3, 6, 20), ("tremole", 2, 4, 10),
    ("buzzoon", 3, 5, 10), ("pipling", 2, 4, 10), ("zapresto", 3, 5, 10),
    ("burrbass", 3, 5, 5), ("timpanite", 4, 5, 5), ("carilloy", 4, 6, 4),
    ("solfawn", 3, 4, 4), ("soarprano", 6, 7, 1), ("rattacca", 6, 7, 1),
])
# Clearing Chord tease (Badge 2): brush walls off the east patch strip.
for y in (15, 16, 17):
    r.obstacles.append((9, y, 'Brush'))
r.music = 'cantorel_bed'
r.npc('rt1_tuner', 4, 7, 'Right', 'npc.trainer', 'rt1_tuner.script.ron', sight=3)
r.npc('rt1_busker', 9, 12, 'Left', 'npc.trainer', 'rt1_busker.script.ron', sight=3)
r.npc('rt1_percussionist', 4, 17, 'Right', 'npc.trainer', 'rt1_percussionist.script.ron', sight=3)
r.npc('rt1_choirboy', 9, 23, 'Left', 'npc.trainer', 'rt1_choirboy.script.ron', sight=3)
# rival ambush mid-route: trigger refires until the fight is WON —
# the script guards on the defeat flag (doc 02 v1.4 #5), and the row
# spans every walkable column so beat 2 cannot be walked around.
for x in range(1, 13):
    r.script_trigger(x, 20, 'rival_ambush.script.ron')
for x in (6, 7):
    r.door(x, 0, 'pausa_village', (9 + (x-6), 12), 'Down')
    r.door(x, 29, 'prelude_town', (10 + (x-6), 1), 'Up')
r.emit(root)

# ---------- prelude_town 22x14 ----------
t = M('prelude_town', 22, 14)
t.rect(3, 3, 18, 10, 'ground', 2)
t.rect(4, 8, 7, 10, 'ground', 4, solid=1)             # mart
t.script_trigger(5, 7, 'mart.script.ron')
t.rect(9, 8, 12, 10, 'ground', 4, solid=1)            # rest stop
t.script_trigger(10, 7, 'rest_stop.script.ron')
t.rect(14, 8, 18, 11, 'ground', 4, solid=1)           # hall 1 facade
t.door(16, 8, 'hall_1', (6, 1), 'Up')
t.music = 'town_prelude'
t.npc('townsfolk_ona', 8, 5, 'Down', 'npc.villager', 'prelude_talk.script.ron', wander=2)
for x in (10, 11):
    t.door(x, 0, 'route_1', (6 + (x-10), 28), 'Down')
t.emit(root)

# ---------- hall_1 12x16 ----------
h = M('hall_1', 12, 16, ground_fill=4)
h.indoor = True
h.rect(2, 3, 9, 12, 'ground', 2)
# the puzzle: two solid pillar rows force an S-path
h.rect(2, 6, 7, 6, 'ground', 4, solid=1)
h.rect(4, 9, 9, 9, 'ground', 4, solid=1)
h.music = 'town_prelude'
h.npc('hall_aide', 3, 4, 'Right', 'npc.trainer', 'hall_aide.script.ron', sight=2)
h.npc('hall_senior', 8, 7, 'Left', 'npc.trainer', 'hall_senior.script.ron', sight=2)
h.npc('dario', 6, 13, 'Down', 'npc.dario', 'dario.script.ron')
h.door(6, 0, 'prelude_town', (16, 7), 'Down')
h.emit(root)
print("maps emitted")

# ===================== P5 — Act 1 maps =====================

# ---------- route_2 (pausa west → arbor vale) 30x14 ----------
r2 = M('route_2', 30, 14)
r2.rect(1, 5, 28, 8, 'ground', 2)
for (x0, x1) in [(4, 8), (14, 18), (22, 26)]:
    r2.rect(x0, 2, x1, 4, 'patch', 1)
    r2.rect(x0, 2, x1, 4, 'ground', 1)
r2.encounters = (12, [
    ("mossoon", 8, 11, 20), ("shrubato", 8, 11, 20), ("petalune", 9, 12, 10),
    ("tremole", 8, 10, 10), ("pipling", 8, 10, 10), ("hummble", 9, 12, 10),
    ("galliard", 10, 12, 5), ("brookoda", 9, 11, 5), ("buzzoon", 9, 11, 4),
    ("cinderle", 9, 12, 4), ("thornata", 12, 13, 1), ("madrigale", 12, 13, 1),
])
r2.night_encounters = (12, [
    ("vesperbat", 9, 12, 20), ("dirgeist", 9, 12, 20), ("mossoon", 8, 11, 10),
    ("fogato", 10, 12, 10), ("mutewing", 10, 13, 10), ("hummble", 9, 12, 10),
    ("glissicle", 9, 11, 5), ("lullaby", 11, 13, 5), ("shrubato", 8, 11, 4),
    ("petalune", 9, 12, 4), ("requiemoth", 12, 13, 1), ("crescendola", 12, 13, 1),
])
r2.music = 'cantorel_bed'
r2.npc('rt2_gardener', 9, 6, 'Right', 'npc.trainer', 'rt2_gardener.script.ron', sight=3)
r2.npc('rt2_courier', 20, 7, 'Left', 'npc.trainer', 'rt2_courier.script.ron', sight=3)
# TACET shipment scene trigger (beat 3): mid-route, refires until won
for y in (5, 6, 7, 8):
    r2.script_trigger(12, y, 'tacet_shipment.script.ron')
for y in (6, 7):
    r2.door(0, y, 'pausa_village', (14, 6 + (y-6)), 'Left')
    r2.door(29, y, 'arbor_vale', (1, 7 + (y-6)), 'Right')
r2.emit(root)

# ---------- arbor_vale (hall 2 town) 24x16 ----------
av = M('arbor_vale', 24, 16)
av.rect(3, 3, 20, 12, 'ground', 2)
av.rect(4, 10, 7, 12, 'ground', 4, solid=1)    # mart
av.script_trigger(5, 9, 'mart.script.ron')
av.rect(9, 10, 12, 12, 'ground', 4, solid=1)   # rest stop
av.script_trigger(10, 9, 'rest_stop.script.ron')
av.rect(15, 10, 19, 13, 'ground', 4, solid=1)  # hall 2
av.door(17, 10, 'hall_2', (6, 1), 'Up')
av.music = 'town_arbor'
av.npc('arbor_elder', 7, 6, 'Down', 'npc.villager', 'arbor_elder.script.ron')
av.npc('echo_keeper_1', 13, 5, 'Down', 'npc.villager', 'anchor_echo_2.script.ron')
for y in (7, 8):
    av.door(0, y, 'route_2', (28, 6 + (y-7)), 'Left')
for x in (11, 12):
    av.door(x, 15, 'route_3', (6 + (x-11), 1), 'Up')
av.emit(root)

# ---------- hall_2 (Mirelle, bloom; vine maze) 12x16 ----------
h2 = M('hall_2', 12, 16, ground_fill=4)
h2.indoor = True
h2.rect(2, 3, 9, 12, 'ground', 2)
h2.rect(2, 6, 6, 6, 'ground', 4, solid=1)
h2.rect(5, 9, 9, 9, 'ground', 4, solid=1)
h2.obstacles.append((8, 6, 'Brush'))            # clearing chord shortcut
h2.music = 'town_arbor'
h2.npc('hall2_pruner', 3, 4, 'Right', 'npc.trainer', 'hall2_pruner.script.ron', sight=2)
h2.npc('hall2_arranger', 8, 7, 'Left', 'npc.trainer', 'hall2_arranger.script.ron', sight=2)
h2.npc('mirelle', 6, 13, 'Down', 'npc.mirelle', 'mirelle.script.ron')
h2.door(6, 0, 'arbor_vale', (17, 9), 'Down')
h2.emit(root)

# ---------- route_3 (arbor → coast fork) 14x26 ----------
r3 = M('route_3', 14, 26)
r3.rect(5, 1, 8, 24, 'ground', 2)
for (y0, y1) in [(4, 8), (13, 17)]:
    r3.rect(2, y0, 4, y1, 'patch', 1)
    r3.rect(2, y0, 4, y1, 'ground', 1)
    r3.rect(9, y0+2, 11, y1+2, 'patch', 1)
r3.encounters = (12, [
    ("galliard", 12, 15, 20), ("shrubato", 12, 14, 20), ("cinderle", 12, 15, 10),
    ("hummble", 12, 15, 10), ("brookoda", 12, 14, 10), ("frostrel", 13, 15, 10),
    ("dunelay", 13, 16, 5), ("ampurr", 13, 16, 5), ("thornata", 14, 16, 4),
    ("brasshorn", 14, 16, 4), ("terracant", 15, 17, 1), ("graviole", 15, 17, 1),
])
r3.music = 'cantorel_bed'
r3.npc('rt3_drover', 4, 6, 'Right', 'npc.trainer', 'rt3_drover.script.ron', sight=3)
r3.npc('rt3_chorister', 9, 15, 'Left', 'npc.trainer', 'rt3_chorister.script.ron', sight=3)
# rival 2 ambush (beat: post-badge-2)
for x in range(1, 13):
    r3.script_trigger(x, 20, 'rival2_ambush.script.ron')
for x in (6, 7):
    r3.door(x, 0, 'arbor_vale', (11 + (x-6), 14), 'Down')
    r3.door(x, 25, 'route_4', (6 + (x-6), 1), 'Up')
# west fork to quiet coast
for y in (12, 13):
    r3.door(0, y, 'quiet_coast', (22, 7 + (y-12)), 'Left')
r3.emit(root)

# ---------- quiet_coast (optional; dead air) 24x12 ----------
qc = M('quiet_coast', 24, 12)
qc.rect(2, 2, 21, 9, 'ground', 1)
qc.rect(1, 1, 22, 3, 'ground', 5)               # the silent sea (water)
for x in range(1, 23):
    qc.coll[1][x] = 0                            # water walkable only via surf
qc.rect(2, 6, 21, 7, 'ground', 2)
# sparse encounters — the song is thin here
qc.rect(15, 4, 18, 5, 'patch', 1)
qc.encounters = (6, [
    ("fogato", 14, 17, 20), ("mutewing", 14, 17, 20), ("gullegro", 13, 16, 10),
    ("anemonet", 13, 16, 10), ("dunelay", 14, 16, 10), ("brinargo", 13, 15, 10),
    ("conchord", 14, 17, 5), ("requiemoth", 16, 18, 5), ("sirenetta", 16, 18, 4),
    ("kelpitan", 15, 17, 4), ("maridian", 16, 18, 1), ("crescendola", 17, 18, 1),
])
qc.npc('fisher_old', 8, 7, 'Down', 'npc.villager', 'quietcoast_fisher.script.ron')
qc.script_trigger(2, 2, 'intervallia_static.script.ron')
qc.npc('echo_keeper_3', 19, 8, 'Left', 'npc.villager', 'anchor_echo_3.script.ron')
for y in (7, 8):
    qc.door(23, y, 'route_3', (1, 12 + (y-7)), 'Right')
qc.emit(root)

# ---------- route_4 (climb to calando) 14x22 ----------
r4 = M('route_4', 14, 22)
r4.rect(5, 1, 8, 20, 'ground', 2)
for (y0, y1) in [(5, 9), (13, 16)]:
    r4.rect(9, y0, 11, y1, 'patch', 1)
    r4.rect(9, y0, 11, y1, 'ground', 1)
r4.encounters = (12, [
    ("cinderle", 14, 17, 20), ("ampurr", 14, 17, 20), ("staccatto", 15, 18, 10),
    ("voltern", 15, 18, 10), ("clockerel", 15, 18, 10), ("dunelay", 14, 17, 10),
    ("brasshorn", 15, 18, 5), ("frostrel", 14, 17, 5), ("forgeling", 16, 18, 4),
    ("vesperbat", 15, 18, 4), ("pyrelodie", 17, 19, 1), ("tidempo", 17, 19, 1),
])
r4.music = 'cantorel_bed'
r4.npc('rt4_stoker', 4, 8, 'Right', 'npc.trainer', 'rt4_stoker.script.ron', sight=3)
r4.npc('rt4_signaler', 9, 12, 'Left', 'npc.trainer', 'rt4_signaler.script.ron', sight=3)
# TACET tuning-yard raid (beat 3b) — doubles grunt fight
for x in range(1, 13):
    r4.script_trigger(x, 18, 'tacet_yard.script.ron')
for x in (6, 7):
    r4.door(x, 0, 'route_3', (6 + (x-6), 24), 'Down')
    r4.door(x, 21, 'port_calando', (11 + (x-6), 1), 'Up')
r4.emit(root)

# ---------- port_calando (hall 3 city) 26x18 ----------
pc = M('port_calando', 26, 18)
pc.rect(2, 2, 23, 15, 'ground', 2)
pc.rect(3, 12, 6, 14, 'ground', 4, solid=1)     # mart
pc.script_trigger(4, 11, 'mart.script.ron')
pc.rect(8, 12, 11, 14, 'ground', 4, solid=1)    # rest stop
pc.script_trigger(9, 11, 'rest_stop.script.ron')
pc.rect(17, 12, 22, 15, 'ground', 4, solid=1)   # hall 3
pc.door(19, 12, 'hall_3', (6, 1), 'Up')
pc.rect(2, 1, 23, 1, 'ground', 5)               # harbor water
pc.music = 'town_calando'
pc.npc('harbor_master', 14, 8, 'Down', 'npc.villager', 'calando_harbor.script.ron')
pc.npc('echo_keeper_pausa', 5, 5, 'Down', 'npc.villager', 'anchor_echo_1.script.ron')
pc.npc('keyshift_buff', 20, 6, 'Down', 'npc.villager', 'keyshift_tutorial.script.ron')
for x in (11, 12):
    pc.door(x, 16, 'route_5', (6 + (x-11), 1), 'Up')
pc.script_trigger(2, 3, 'skalden_boat.script.ron')  # the quay (P8)
pc.rect(1, 3, 2, 4, 'ground', 2)  # quay approach
pc.rect(11, 1, 12, 1, 'ground', 2)  # path gap to the south doors
for x in (11, 12):
    pc.door(x, 0, 'route_4', (6 + (x-11), 20), 'Down')
pc.emit(root)

# ---------- hall_3 (doubles hall; Lull encounter after) 14x18 ----------
h3 = M('hall_3', 14, 18, ground_fill=4)
h3.indoor = True
h3.rect(2, 3, 11, 14, 'ground', 2)
h3.rect(2, 7, 8, 7, 'ground', 4, solid=1)
h3.rect(5, 11, 11, 11, 'ground', 4, solid=1)
h3.music = 'town_calando'
h3.npc('hall3_duo_a', 3, 5, 'Right', 'npc.trainer', 'hall3_duo.script.ron', sight=2)
h3.npc('hall3_duo_b', 10, 9, 'Left', 'npc.trainer', 'hall3_duo2.script.ron', sight=2)
h3.npc('maestro_stelt', 7, 15, 'Down', 'npc.dario', 'stelt.script.ron')
h3.script_trigger(6, 2, 'lull_scene.script.ron')
h3.door(6, 0, 'port_calando', (19, 11), 'Down')
h3.emit(root)
print("act-1 maps emitted")

# ===================== P6 — Acts 2–3 maps =====================

# ---------- route_5 (calando → voltaccia) 14x24 ----------
r5 = M('route_5', 14, 24)
r5.rect(5, 1, 8, 22, 'ground', 2)
for (y0, y1) in [(4, 8), (14, 18)]:
    r5.rect(2, y0, 4, y1, 'patch', 1)
    r5.rect(2, y0, 4, y1, 'ground', 1)
r5.encounters = (12, [
    ("ampurr", 18, 21, 20), ("ratchetta", 18, 21, 20), ("voltern", 19, 22, 10),
    ("staccatto", 19, 22, 10), ("smogturne", 20, 23, 10), ("dynamotif", 20, 23, 10),
    ("clockerel", 19, 22, 5), ("brasshorn", 19, 22, 5), ("forgeling", 20, 23, 4),
    ("pistonna", 21, 23, 4), ("dynamaestro", 30, 32, 1), ("cadenzear", 24, 26, 1),
])
r5.music = 'cantorel_bed'
r5.npc('rt5_linesman', 4, 7, 'Right', 'npc.trainer', 'rt5_linesman.script.ron', sight=3)
r5.npc('rt5_foreman', 9, 16, 'Left', 'npc.trainer', 'rt5_foreman.script.ron', sight=3)
for x in (6, 7):
    r5.door(x, 0, 'port_calando', (11 + (x-6), 16), 'Down')
    r5.door(x, 23, 'voltaccia', (11 + (x-6), 1), 'Up')
r5.emit(root)

# ---------- voltaccia (hall 4) 24x16 ----------
vt = M('voltaccia', 24, 16)
vt.rect(2, 2, 21, 13, 'ground', 2)
vt.rect(3, 10, 6, 12, 'ground', 4, solid=1)     # mart
vt.script_trigger(4, 9, 'mart.script.ron')
vt.rect(8, 10, 11, 12, 'ground', 4, solid=1)    # rest
vt.script_trigger(9, 9, 'rest_stop.script.ron')
vt.rect(14, 10, 19, 13, 'ground', 4, solid=1)   # hall 4
vt.door(16, 10, 'hall_4', (6, 1), 'Up')
vt.weather = 'Heatwave'                          # industrial heat zone
vt.music = 'town_calando'
vt.npc('echo_keeper_4', 5, 5, 'Down', 'npc.villager', 'anchor_echo_4.script.ron')
vt.npc('volt_worker', 13, 6, 'Down', 'npc.villager', 'voltaccia_talk.script.ron')
for y in (6, 7):
    vt.door(23, y, 'route_5b', (1, y), 'Right')
vt.rect(11, 1, 12, 1, 'ground', 2)  # path gap to the south doors
for x in (11, 12):
    vt.door(x, 0, 'route_5', (6 + (x-11), 22), 'Down')
vt.emit(root)

# ---------- hall_4 (Rhea, volt; basement door beat 6) 12x18 ----------
h4 = M('hall_4', 12, 18, ground_fill=4)
h4.indoor = True
h4.rect(2, 3, 9, 14, 'ground', 2)
h4.rect(2, 6, 7, 6, 'ground', 4, solid=1)
h4.rect(4, 10, 9, 10, 'ground', 4, solid=1)
h4.music = 'town_calando'
h4.npc('hall4_winder', 3, 4, 'Right', 'npc.trainer', 'hall4_winder.script.ron', sight=2)
h4.npc('hall4_coiler', 7, 8, 'Right', 'npc.trainer', 'hall4_coiler.script.ron', sight=2)
h4.npc('rhea', 6, 15, 'Down', 'npc.dario', 'rhea.script.ron')
# beat 6: the maintenance door (post-badge-4) — basement trigger
h4.script_trigger(2, 14, 'maintenance_door.script.ron')
h4.door(6, 0, 'voltaccia', (16, 9), 'Down')
h4.emit(root)

# ---------- route_5b (voltaccia → hollowfen) 26x12 ----------
r5b = M('route_5b', 26, 12)
r5b.rect(1, 5, 24, 8, 'ground', 2)
r5b.rect(8, 2, 12, 4, 'patch', 1)
r5b.rect(16, 2, 20, 4, 'patch', 1)
r5b.encounters = (12, [
    ("croakorus", 21, 24, 20), ("mirefall", 21, 24, 20), ("fenadenza", 22, 25, 10),
    ("willowisp", 22, 25, 10), ("anemonet", 21, 24, 10), ("buzzoon", 20, 23, 10),
    ("mutewing", 22, 25, 5), ("bogritone", 23, 26, 5), ("lullaby", 23, 26, 4),
    ("kelpitan", 22, 25, 4), ("vespernox", 26, 28, 1), ("velvetide", 25, 27, 1),
])
r5b.music = 'cantorel_bed'
r5b.npc('rt5b_bogger', 6, 6, 'Right', 'npc.trainer', 'rt5b_bogger.script.ron', sight=3)
r5b.npc('rt5b_lampman', 18, 7, 'Left', 'npc.trainer', 'rt5b_lampman.script.ron', sight=3)
for y in (6, 7):
    r5b.door(0, y, 'voltaccia', (22, y), 'Left')
    r5b.door(25, y, 'hollowfen', (1, 6 + (y-6)), 'Right')
r5b.emit(root)

# ---------- hollowfen (hall 5, derelict opera house) 22x14 ----------
hf = M('hollowfen', 22, 14)
hf.rect(2, 2, 19, 11, 'ground', 2)
hf.rect(3, 8, 6, 10, 'ground', 4, solid=1)      # mart
hf.script_trigger(4, 7, 'mart.script.ron')
hf.rect(8, 8, 11, 10, 'ground', 4, solid=1)     # rest
hf.script_trigger(9, 7, 'rest_stop.script.ron')
hf.rect(14, 8, 18, 11, 'ground', 4, solid=1)    # hall 5 (opera house)
hf.door(16, 8, 'hall_5', (6, 1), 'Up')
hf.music = 'town_arbor'
hf.npc('echo_keeper_5', 6, 4, 'Down', 'npc.villager', 'anchor_echo_5.script.ron')
hf.npc('fen_elder', 13, 5, 'Down', 'npc.villager', 'hollowfen_talk.script.ron')
for y in (6, 7):
    hf.door(0, y, 'route_5b', (24, 6 + (y-6)), 'Left')
for x in (10, 11):
    hf.door(x, 13, 'graven_pass', (6 + (x-10), 1), 'Up')
hf.emit(root)

# ---------- hall_5 (Maren, phantom; the letter, beat 7) 12x16 ----------
h5 = M('hall_5', 12, 16, ground_fill=4)
h5.indoor = True
h5.rect(2, 3, 9, 12, 'ground', 2)
h5.rect(4, 6, 9, 6, 'ground', 4, solid=1)
h5.rect(2, 9, 7, 9, 'ground', 4, solid=1)
h5.dark = True                                   # derelict opera house
h5.music = None
h5.npc('hall5_usher', 8, 4, 'Left', 'npc.trainer', 'hall5_usher.script.ron', sight=2)
h5.npc('hall5_prompter', 3, 7, 'Right', 'npc.trainer', 'hall5_prompter.script.ron', sight=2)
h5.npc('maren', 6, 13, 'Down', 'npc.dario', 'maren.script.ron')
h5.door(6, 0, 'hollowfen', (16, 7), 'Down')
h5.emit(root)

# ---------- graven_pass (hall 6 town + anchor attack, beat 8) 14x28 ----------
gp = M('graven_pass', 14, 28)
gp.rect(5, 1, 8, 26, 'ground', 2)
gp.rect(2, 6, 4, 10, 'patch', 1)
gp.rect(9, 14, 11, 18, 'patch', 1)
gp.encounters = (12, [
    ("gravelody", 24, 27, 20), ("dunelay", 23, 26, 20), ("shiverine", 24, 27, 10),
    ("frostrel", 23, 26, 10), ("emberlith", 25, 28, 10), ("brasshorn", 23, 26, 10),
    ("graviole", 24, 27, 5), ("terracant", 25, 28, 5), ("avalanche", 26, 29, 4),
    ("gravoross", 33, 35, 4), ("zephyrune", 27, 29, 1), ("cantavella", 40, 40, 1),
])
gp.music = 'cantorel_bed'
gp.rect(4, 20, 7, 23, 'ground', 4, solid=1)      # hall 6 (x8 lane stays open)
gp.door(6, 20, 'hall_6', (6, 1), 'Up')
gp.script_trigger(5, 12, 'rest_stop.script.ron') # waystation
gp.script_trigger(6, 25, 'cantavella_static.script.ron')
gp.npc('echo_keeper_6', 9, 9, 'Left', 'npc.villager', 'anchor_echo_6.script.ron')
# beat 8: the anchor attack scene row
for x in range(1, 13):
    gp.script_trigger(x, 16, 'graven_anchor.script.ron')
for x in (6, 7):
    gp.door(x, 0, 'hollowfen', (10 + (x-6), 12), 'Down')
    gp.door(x, 27, 'frostine', (11 + (x-6), 1), 'Up')
gp.emit(root)

# ---------- hall_6 (Orsk, stone; HP-stall) 12x14 ----------
h6 = M('hall_6', 12, 14, ground_fill=4)
h6.indoor = True
h6.rect(2, 3, 9, 11, 'ground', 2)
h6.rect(2, 7, 6, 7, 'ground', 4, solid=1)
h6.music = 'cantorel_bed'
h6.npc('hall6_mason', 8, 5, 'Left', 'npc.trainer', 'hall6_mason.script.ron', sight=2)
h6.npc('orsk', 6, 12, 'Down', 'npc.dario', 'orsk.script.ron')
h6.door(6, 0, 'graven_pass', (6, 19), 'Down')
h6.emit(root)

# ---------- frostine (hall 7, glacier conservatoire) 24x14 ----------
fr = M('frostine', 24, 14)
fr.rect(2, 2, 21, 11, 'ground', 2)
fr.rect(3, 8, 6, 10, 'ground', 4, solid=1)
fr.script_trigger(4, 7, 'mart.script.ron')
fr.rect(8, 8, 11, 10, 'ground', 4, solid=1)
fr.script_trigger(9, 7, 'rest_stop.script.ron')
fr.rect(14, 8, 19, 11, 'ground', 4, solid=1)    # hall 7
fr.door(16, 8, 'hall_7', (6, 1), 'Up')
fr.weather = 'Flurry'
fr.music = 'town_prelude'
fr.npc('echo_keeper_7', 6, 4, 'Down', 'npc.villager', 'anchor_echo_7.script.ron')
fr.npc('frost_archivist', 13, 5, 'Down', 'npc.villager', 'frostine_talk.script.ron')
for x in (11, 12):
    fr.door(x, 0, 'graven_pass', (6 + (x-11), 26), 'Down')
    fr.door(x, 13, 'route_6', (6 + (x-11), 1), 'Up')
fr.emit(root)

# ---------- hall_7 (Ilva, frost; hardest pre-league) 12x16 ----------
h7 = M('hall_7', 12, 16, ground_fill=4)
h7.indoor = True
h7.rect(2, 3, 9, 13, 'ground', 2)
h7.rect(2, 6, 7, 6, 'ground', 4, solid=1)
h7.rect(4, 10, 9, 10, 'ground', 4, solid=1)
h7.weather = 'Flurry'
h7.music = 'town_prelude'
h7.npc('hall7_skater', 3, 4, 'Right', 'npc.trainer', 'hall7_skater.script.ron', sight=2)
h7.npc('hall7_carver', 7, 8, 'Right', 'npc.trainer', 'hall7_carver.script.ron', sight=2)
h7.npc('ilva', 6, 14, 'Down', 'npc.dario', 'ilva.script.ron')
h7.door(6, 0, 'frostine', (16, 7), 'Down')
h7.emit(root)

# ---------- route_6 (frostine → cadenza) 14x20 ----------
r6 = M('route_6', 14, 20)
r6.rect(5, 1, 8, 18, 'ground', 2)
r6.rect(9, 5, 11, 9, 'patch', 1)
r6.encounters = (12, [
    ("shiverine", 28, 31, 20), ("staccatto", 27, 30, 20), ("cadenzear", 29, 32, 10),
    ("galegant", 28, 31, 10), ("vesperbat", 27, 30, 10), ("clockerel", 27, 30, 10),
    ("zephyrune", 29, 32, 5), ("maestrina", 30, 33, 5), ("orchestrios", 31, 34, 4),
    ("nullavoce", 31, 34, 4), ("requiemoth", 30, 33, 1), ("intervallia", 45, 45, 1),
])
r6.music = 'cantorel_bed'
r6.npc('rt6_virtuoso', 4, 8, 'Right', 'npc.trainer', 'rt6_virtuoso.script.ron', sight=3)
r6.npc('rt6_envoy', 9, 13, 'Left', 'npc.trainer', 'rt6_envoy.script.ron', sight=3)
for x in (6, 7):
    r6.door(x, 0, 'frostine', (11 + (x-6), 12), 'Down')
    r6.door(x, 19, 'cadenza_city', (12 + (x-6), 2), 'Up')
r6.emit(root)

# ---------- cadenza_city (hall 8, capital) 26x18 ----------
cz = M('cadenza_city', 26, 18)
cz.rect(2, 2, 23, 15, 'ground', 2)
cz.rect(3, 12, 6, 14, 'ground', 4, solid=1)
cz.script_trigger(4, 11, 'mart.script.ron')
cz.rect(8, 12, 11, 14, 'ground', 4, solid=1)
cz.script_trigger(9, 11, 'rest_stop.script.ron')
cz.rect(15, 12, 21, 15, 'ground', 4, solid=1)   # hall 8
cz.door(18, 12, 'hall_8', (6, 1), 'Up')
cz.music = 'town_calando'
cz.npc('echo_keeper_8', 5, 5, 'Down', 'npc.villager', 'anchor_echo_8.script.ron')
cz.npc('reed_capital', 13, 7, 'Down', 'npc.reed', 'reed_confession.script.ron')
cz.npc('cade_capital', 17, 6, 'Down', 'npc.villager2', 'cade_plea.script.ron')
cz.rect(12, 1, 13, 1, 'ground', 2)  # path gap to the south doors
for x in (12, 13):
    cz.door(x, 0, 'route_6', (6 + (x-12), 18), 'Down')
# the spire (Quartet) opens with badge 8
for x in (24, 24):
    pass
cz.door(24, 9, 'quartet_spire', (5, 1), 'Right')
cz.door(7, 10, 'encore_hall', (4, 1), 'Up')
for x in range(6, 9):
    cz.script_trigger(x, 9, 'encore_gate.script.ron')
cz.emit(root)

# ---------- hall_8 (Calder, alloy) 12x16 ----------
h8 = M('hall_8', 12, 16, ground_fill=4)
h8.indoor = True
h8.rect(2, 3, 9, 13, 'ground', 2)
h8.rect(2, 6, 7, 6, 'ground', 4, solid=1)
h8.rect(4, 10, 9, 10, 'ground', 4, solid=1)
h8.music = 'battle_hall'
h8.npc('hall8_registrar', 3, 4, 'Right', 'npc.trainer', 'hall8_registrar.script.ron', sight=2)
h8.npc('calder', 6, 14, 'Down', 'npc.dario', 'calder.script.ron')
h8.door(6, 0, 'cadenza_city', (18, 11), 'Down')
h8.emit(root)

# ---------- quartet_spire (4 ascending fights) 10x30 ----------
qs = M('quartet_spire', 10, 30, ground_fill=4)
qs.indoor = True
qs.rect(3, 1, 6, 28, 'ground', 2)
qs.music = 'battle_hall'
for x in (3, 4, 5, 6):
    qs.script_trigger(x, 2, 'spire_gate.script.ron')
qs.npc('quartet_1', 4, 6, 'Down', 'npc.dario', 'quartet_1.script.ron')
qs.npc('quartet_2', 5, 12, 'Down', 'npc.dario', 'quartet_2.script.ron')
qs.npc('quartet_3', 4, 18, 'Down', 'npc.dario', 'quartet_3.script.ron')
qs.npc('quartet_4', 5, 24, 'Down', 'npc.dario', 'quartet_4.script.ron')
# the empty Soloist stage: the floor opens (descent trigger)
qs.script_trigger(6, 26, 'primavoce_static.script.ron')
qs.script_trigger(4, 28, 'soloist_stage.script.ron')
qs.script_trigger(5, 28, 'soloist_stage.script.ron')
qs.door(5, 0, 'cadenza_city', (23, 9), 'Down')
qs.emit(root)

# ---------- the_vault (descent; dead air; endings) 10x34 ----------
vault = M('the_vault', 10, 34, ground_fill=4)
vault.indoor = True
vault.rect(3, 1, 6, 32, 'ground', 2)
vault.dark = True
vault.music = None                               # the thinning music
vault.npc('vault_guard_1', 4, 6, 'Down', 'npc.dario', 'vault_guard_1.script.ron')
vault.npc('vault_admin_hush', 5, 12, 'Down', 'npc.villager', 'vault_hush.script.ron')
vault.npc('vault_guard_2', 4, 18, 'Down', 'npc.dario', 'vault_guard_2.script.ron')
vault.npc('vault_vesper', 5, 24, 'Down', 'npc.villager', 'vault_vesper.script.ron')
# the bottom: Aria and the held chord — the ending choice
vault.script_trigger(3, 29, 'taciturn_static.script.ron')
vault.script_trigger(4, 31, 'aria_finale.script.ron')
vault.script_trigger(5, 31, 'aria_finale.script.ron')
vault.door(5, 0, 'quartet_spire', (5, 27), 'Down')
vault.emit(root)
print("acts 2-3 maps emitted")

# ---------- encore_hall (P7 battle tower) 10x12 ----------
eh = M('encore_hall', 10, 12, ground_fill=4)
eh.indoor = True
eh.rect(2, 2, 7, 9, 'ground', 2)
eh.music = 'battle_hall'
eh.npc('encore_marshal', 4, 8, 'Down', 'npc.dario', 'encore_marshal.script.ron')
eh.door(4, 0, 'cadenza_city', (7, 9), 'Down')
eh.emit(root)
print("encore hall emitted")

# ===================== P8 — Skalden pack =====================
skroot = 'content/regions/skalden/maps'
import os
os.makedirs(skroot, exist_ok=True)

def sk_town(name, hall_n, music, with_quay=False, east=None, west=None, field=None, enc=None):
    t = M(name, 24, 14)
    t.rect(2, 2, 21, 11, 'ground', 2)
    t.rect(3, 8, 6, 10, 'ground', 4, solid=1)    # mart
    t.script_trigger(4, 7, 'mart.script.ron')
    t.rect(8, 8, 11, 10, 'ground', 4, solid=1)   # rest
    t.script_trigger(9, 7, 'rest_stop.script.ron')
    t.rect(14, 9, 19, 12, 'ground', 4, solid=1)  # hall (door below at y8)
    t.door(16, 9, f'skald_hall_{hall_n}', (6, 1), 'Up')
    # the door STEP the test uses is (16,8) → make it a warp tile too
    t.door(16, 8, f'skald_hall_{hall_n}', (6, 1), 'Up')
    t.music = music
    t.npc('verse_keeper_%d' % hall_n, 5, 5, 'Down', 'npc.villager', 'verse_talk.script.ron')
    if field:
        t.rect(field[0], field[1], field[2], field[3], 'patch', 1)
        t.encounters = enc
    if with_quay:
        t.script_trigger(2, 3, 'quay_return.script.ron')
    if east:
        t.door(23, 7, east, (1, 7), 'Right')
    if west:
        t.door(0, 7, west, (22, 7), 'Left')
    t.emit(skroot)
    return t

sk_town('port_skald', 1, 'town_skald', with_quay=True, east='varde',
        field=(2, 2, 5, 4), enc=(12, [
            ("skerryn", 18, 22, 20), ("gullnote", 18, 22, 20), ("brinemare", 19, 23, 10),
            ("eiderdun", 19, 23, 10), ("saltlick", 20, 24, 10), ("turfowl", 19, 23, 10),
            ("brackaw", 22, 25, 5), ("peatling", 19, 22, 5), ("nokkelpie", 24, 27, 4),
            ("glimmwyrm", 24, 27, 4), ("fjordrake", 28, 30, 1), ("skerrald", 26, 28, 1),
        ]))
sk_town('varde', 2, 'town_varde', east='fenwick_hollow', west='port_skald',
        field=(2, 2, 5, 4), enc=(12, [
            ("spindrel", 22, 26, 20), ("fiddlefox", 22, 26, 20), ("cobbelt", 23, 27, 10),
            ("turfowl", 22, 26, 10), ("emberjarl", 23, 27, 10), ("waeverin", 24, 28, 10),
            ("gullnote", 22, 25, 5), ("mireling", 23, 26, 5), ("reynardeau", 28, 31, 4),
            ("runolf", 29, 32, 4), ("henge", 30, 32, 1), ("loomgarde", 30, 32, 1),
        ]))
sk_town('fenwick_hollow', 3, 'town_skald', east='kraghorn', west='varde',
        field=(2, 2, 5, 4), enc=(12, [
            ("peatling", 26, 30, 20), ("mireling", 26, 30, 20), ("bogboar", 27, 31, 10),
            ("willowail", 27, 31, 10), ("brackaw", 27, 31, 10), ("turfowl", 26, 29, 10),
            ("hulderbuck", 30, 33, 5), ("dirgelk", 29, 32, 5), ("barrowmoss", 30, 33, 4),
            ("eiderdun", 27, 30, 4), ("nokkelpie", 30, 33, 1), ("willowail", 31, 34, 1),
        ]))
sk_town('kraghorn', 4, 'town_varde', west='fenwick_hollow',
        field=(2, 2, 5, 4), enc=(12, [
            ("screel", 30, 34, 20), ("frostfiddle", 30, 34, 20), ("rimeram", 31, 35, 10),
            ("voltvarg", 31, 35, 10), ("kragveld", 33, 36, 10), ("jokulhorn", 33, 36, 10),
            ("aurorale", 34, 37, 5), ("tindrelod", 34, 37, 5), ("skarnbjorn", 34, 37, 4),
            ("dirgelk", 32, 35, 4), ("velkomma", 33, 36, 1), ("emberjarl", 32, 35, 1),
        ]))

for n in range(1, 5):
    h = M(f'skald_hall_{n}', 12, 12, ground_fill=4)
    h.indoor = True
    h.rect(2, 2, 9, 10, 'ground', 2)
    h.music = 'battle_skalden_hall'
    h.npc(f'skald_maestro_{n}', 6, 10, 'Down', 'npc.dario', f'skald_maestro_{n}.script.ron')
    town = ['port_skald', 'varde', 'fenwick_hollow', 'kraghorn'][n-1]
    h.door(6, 0, town, (16, 7), 'Down')
    h.emit(skroot)
print("skalden maps emitted")
