#!/usr/bin/env python3
"""Generates the Rivet prototype screens that share one structure.

Each screen is a spec: header (title, back target, rail item) plus rows.
Every row that shows a value carries `src`: where the real app reads it.
`data-sources.md` is produced from the same specs.
"""
import json, os, sys

_HERE = os.path.dirname(os.path.abspath(__file__))
# Scratch layout keeps screens in ./project; the repo copy keeps them next to the script.
OUT = os.path.join(_HERE, 'project') if os.path.isdir(os.path.join(_HERE, 'project')) else _HERE

# --- icons (24px stroke) -----------------------------------------------------
C = "M12 3a9 9 0 1 0 0 18a9 9 0 1 0 0-18"
ICONS = {
    'home': ["M3.5 10.2 12 3.5l8.5 6.7V19a1.5 1.5 0 0 1-1.5 1.5h-4v-6h-6v6H5A1.5 1.5 0 0 1 3.5 19z"],
    'control': ["M4 7h10", "M18 7h2", "M16 5a2 2 0 1 0 0 4a2 2 0 1 0 0-4", "M4 17h2", "M10 17h10", "M8 15a2 2 0 1 0 0 4a2 2 0 1 0 0-4"],
    'tools': ["M14.7 6.3a4 4 0 0 0-5.4 5.2L3.5 17.3a1.8 1.8 0 0 0 2.5 2.5l5.8-5.8a4 4 0 0 0 5.2-5.4l-2.4 2.4-2.3-.4-.4-2.3z"],
    'settings': ["M12 9a3 3 0 1 0 0 6a3 3 0 1 0 0-6", "M19.4 13.5a7.6 7.6 0 0 0 0-3l2-1.6-2-3.4-2.4.9a7.5 7.5 0 0 0-2.6-1.5L14 2.5h-4l-.4 2.4A7.5 7.5 0 0 0 7 6.4l-2.4-.9-2 3.4 2 1.6a7.6 7.6 0 0 0 0 3l-2 1.6 2 3.4 2.4-.9a7.5 7.5 0 0 0 2.6 1.5l.4 2.4h4l.4-2.4a7.5 7.5 0 0 0 2.6-1.5l2.4.9 2-3.4z"],
    'bell': ["M6 8a6 6 0 1 1 12 0c0 7 3 8.5 3 8.5H3S6 15 6 8", "M10 20a2 2 0 0 0 4 0"],
    'flame': ["M12 3c2 3 5 5 5 9a5 5 0 0 1-10 0c0-2 1-3.5 2-4.5 0 2 1 3 2 3 0-3-1-5 1-7.5z"],
    'move': ["M12 3v18", "M3 12h18", "m9 6 3-3 3 3", "m9 18 3 3 3-3", "m6 9-3 3 3 3", "m18 9 3 3-3 3"],
    'extruder': ["M8 3h8v7H8z", "M10 10v4l2 3 2-3v-4", "M12 17v4"],
    'fan': ["M12 10.2a1.8 1.8 0 1 0 0 3.6a1.8 1.8 0 1 0 0-3.6", "M12 10.2c0-4 1-6.7 3.4-6.7 2.3 0 2.6 3.2.2 5.2", "M13.6 12.9c3.5 2 5.3 4.2 4.1 6.3-1.2 2-4.1.7-4.7-2.4", "M10.4 12.9c-3.5 2-6.4 2.5-7.5.4-1.2-2 1.4-3.9 4.4-2.9"],
    'bulb': ["M9 18h6", "M10 21h4", "M12 3a6 6 0 0 0-3.5 10.9c.6.5 1 1.2 1 2V16h5v-.1c0-.8.4-1.5 1-2A6 6 0 0 0 12 3z"],
    'gauge': ["M12 13.5 16 9", "M4.2 17.5a9 9 0 1 1 15.6 0"],
    'zoff': ["M9 3h6", "M10 3v6l2 3 2-3V3", "M3 19h18", "M12 14v2"],
    'plug': ["M9 2v6", "M15 2v6", "M6 8h12v4a6 6 0 0 1-12 0z", "M12 18v4"],
    'calib': [C, "M12 9a3 3 0 1 0 0 6a3 3 0 1 0 0-6", "M12 1v4M12 19v4M1 12h4M19 12h4"],
    'macro': ["M5 4h14a2 2 0 0 1 2 2v12a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2z", "m7 9 3 3-3 3", "M13 15h4"],
    'console': ["M4 5h16v14H4z", "m7 10 2 2-2 2", "M11 14h5"],
    'history': ["M3 12a9 9 0 1 0 3-6.7", "M3 4v4h4", "M12 7v5l3 2"],
    'camera': ["M4 7h3l2-2h6l2 2h3v12H4z", "M12 10a3 3 0 1 0 0 6a3 3 0 1 0 0-6"],
    'film': ["M4 4h16v16H4z", "M8 4v16M16 4v16M4 8h4M4 12h4M4 16h4M16 8h4M16 12h4M16 16h4"],
    'wrench': ["M14.7 6.3a4 4 0 0 0-5.4 5.2L3.5 17.3a1.8 1.8 0 0 0 2.5 2.5l5.8-5.8a4 4 0 0 0 5.2-5.4l-2.4 2.4-2.3-.4-.4-2.3z"],
    'sensor': ["M12 14V4a2 2 0 0 0-4 0v10a4 4 0 1 0 4 0", "M16 6h4M16 10h4"],
    'limits': ["M4 18a8 8 0 1 1 16 0", "M12 18l4-6", "M4 18h16"],
    'drop': ["M12 3s6 6.5 6 11a6 6 0 0 1-12 0c0-4.5 6-11 6-11z"],
    'wifi': ["M5 12.5a10 10 0 0 1 14 0", "M8.5 16a5 5 0 0 1 7 0", "M2 9a15 15 0 0 1 20 0", "M12 19.5h.01"],
    'screen': ["M12 8a4 4 0 1 0 0 8a4 4 0 1 0 0-8", "M12 2v2M12 20v2M2 12h2M20 12h2", "M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"],
    'lock': ["M7 11h10a2 2 0 0 1 2 2v6a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2v-6a2 2 0 0 1 2-2z", "M8 11V7a4 4 0 0 1 8 0v4"],
    'sound': ["M11 5 6 9H3v6h3l5 4z", "M15.5 8.5a5 5 0 0 1 0 7", "M18.5 5.5a9 9 0 0 1 0 13"],
    'lang': [C, "M3 12h18", "M12 3c2.5 2.5 3.8 5.5 3.8 9s-1.3 6.5-3.8 9c-2.5-2.5-3.8-5.5-3.8-9s1.3-6.5 3.8-9z"],
    'shield': ["M12 3 4 6v6c0 4.5 3.4 8 8 9 4.6-1 8-4.5 8-9V6z", "m9 12 2 2 4-4"],
    'printer': ["M6 9V3h12v6", "M6 18H4v-7h16v7h-2", "M6 14h12v7H6z"],
    'update': ["M12 19V5", "m6 11 6-6 6 6", "M5 21h14"],
    'chip': ["M8 6h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2z", "M9.5 2v4M14.5 2v4M9.5 18v4M14.5 18v4", "M2 9.5h4M2 14.5h4M18 9.5h4M18 14.5h4"],
    'info': [C, "M12 11v6", "M12 7.5h.01"],
    'mesh': ["M3 7l9-4 9 4-9 4z", "M3 12l9 4 9-4", "M3 17l9 4 9-4"],
    'screw': ["M12 3a4 4 0 1 0 0 8a4 4 0 1 0 0-8", "M12 11v10", "M10 14h4M10 17h4"],
    'probe': ["M10 3h4v8l-2 3-2-3z", "M4 20h16", "M12 16v1"],
    'wave': ["M2 12c2-6 4-6 6 0s4 6 6 0 4-6 6 0"],
    'belt': ["M5 8a4 4 0 1 0 0 8", "M19 8a4 4 0 1 1 0 8", "M5 8h14M5 16h14"],
    'pa': ["M4 18 10 6l4 8 2-4 4 8"],
    'scale': ["M4 20h16", "M6 20l2-9h8l2 9", "M12 11V5", "M9 5h6"],
    'driver': ["M5 5h14v14H5z", "M9 9h6v6H9z", "M3 9h2M3 15h2M19 9h2M19 15h2"],
    'power': ["M12 3v8", "M6.3 7.3a8 8 0 1 0 11.4 0"],
    'trash': ["M4 7h16", "M9 7V4h6v3", "M6 7l1 13h10l1-13"],
    'folder': ["M3.5 7.5A2 2 0 0 1 5.5 5.5h4l2 2.5h7a2 2 0 0 1 2 2v7.5a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z"],
    'queue': ["M4 6h16M4 12h16M4 18h10"],
    'pause': ["M8 5v14M16 5v14"],
    'layers': ["M12 3 3 8l9 5 9-5z", "M3 13l9 5 9-5"],
    'spool': [C, "M12 9a3 3 0 1 0 0 6a3 3 0 1 0 0-6", "M12 3v6M12 15v6"],
    'thermo': ["M12 14V4a2 2 0 0 0-4 0v10a4 4 0 1 0 4 0"],
    'graph': ["M3 3v18h18", "M7 15l4-5 3 3 5-6"],
    'stop': ["M8 7h8a1 1 0 0 1 1 1v8a1 1 0 0 1-1 1H8a1 1 0 0 1-1-1V8a1 1 0 0 1 1-1z"],
    'check': ["M5 12.5l4.5 4.5L19 7"],
    'warn': ["M12 3 2 20h20z", "M12 10v4", "M12 17h.01"],
    'retract': ["M12 20V8", "m7 12 5-5 5 5", "M5 4h14"],
    'save': ["M5 4h11l3 3v13H5z", "M8 4v5h7V4", "M8 20v-6h8v6"],
    'pin': ["M12 3v10", "M8 13h8", "M12 13v8"],
    'dryer': ["M4 18h16", "M7 14c0-2 2-2 2-4s-2-2-2-4", "M12 14c0-2 2-2 2-4s-2-2-2-4", "M17 14c0-2 2-2 2-4s-2-2-2-4"],
    'load': ["M12 3v9", "m8 8 4 4 4-4", "M8 15h8l-2 4h-4z"],
    'unload': ["M12 12V3", "m8 7 4-4 4 4", "M8 15h8l-2 4h-4z"],
    'swap': ["M4 9h13", "m14 5 4 4-4 4", "M20 15H7", "m10 19-4-4 4-4"],
    'service': ["M4 6h16v14H4z", "M8 3v4M16 3v4", "M4 10h16", "m9 15 2 2 4-4"],
    'bolt': ["M13 2 4 14h7l-1 8 9-12h-7z"],
    'filter': ["M3 5h18l-7 8v6l-4 2v-8z"],
}

def svg(name, size=26, color='currentColor', sw='1.9'):
    paths = ''.join('<path d="%s"></path>' % d for d in ICONS[name])
    return ('<svg width="%d" height="%d" viewBox="0 0 24 24" fill="none" stroke="%s" stroke-width="%s" '
            'stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">%s</svg>') % (size, size, color, sw, paths)

HEAD = '''<!doctype html>
<html lang="ru">
<head>
<meta charset="utf-8">
<title>%s — Rivet</title>
<script src="./support.js"></script>
</head>
<body>
<x-dc>
<helmet>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700;800&amp;display=swap">
<style>
body{margin:0}
a{color:#2dd4bf;text-decoration:none}a:hover{color:#5eead4}
.scroll{overflow-y:scroll;scrollbar-width:thin;scrollbar-color:#4a525c #15181d;padding-right:10px}
.scroll::-webkit-scrollbar{width:8px}
.scroll::-webkit-scrollbar-track{background:#15181d;border-radius:4px}
.scroll::-webkit-scrollbar-thumb{background:#4a525c;border-radius:4px}
</style>
</helmet>
'''

TAIL = '''</x-dc>
<script type="text/x-dc" data-dc-script data-props='%s'>
%s
</script>
</body>
</html>
'''

def root_open(rail):
    return ('<div style="position: relative; width: 800px; height: 480px; display: flex; background: #0b0d10; color: #f4f6f8; '
            'font-family: Inter, system-ui, sans-serif; overflow: hidden">\n'
            '  <dc-import name="NavRail" active="%s" hint-size="88px,480px"></dc-import>\n') % rail

def header(title, back, right=''):
    b = ''
    if back:
        b = ('<a href="%s" aria-label="Назад" style="width: 64px; height: 64px; flex: none; border-radius: 20px; background: #15181d; '
             'color: #f4f6f8; display: flex; align-items: center; justify-content: center">'
             '<svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" '
             'stroke-linejoin="round" aria-hidden="true"><path d="m15 18-6-6 6-6"></path></svg></a>') % back
    return ('    <div style="height: 64px; flex: none; display: flex; align-items: center; gap: 14px">%s'
            '<h1 style="margin: 0; flex: 1; font-size: 26px; font-weight: 800; letter-spacing: -0.02em">%s</h1>%s</div>\n') % (b, title, right)

def write(name, title, body, js, props=None):
    p = {'$preview': {'width': 800, 'height': 480}}
    if props:
        p = dict(props, **p)
    html = HEAD % title + body + TAIL % (json.dumps(p, ensure_ascii=False).replace("'", '&#39;'), js)
    with open(os.path.join(OUT, name + '.dc.html'), 'w') as f:
        f.write(html)

SOURCES = []  # (screen, label, source)

def note_src(screen, label, src):
    if src:
        SOURCES.append((screen, label, src))

# --- hub screens (tile grids) ------------------------------------------------
def hub(name, title, rail, tiles, cols=4, back=None):
    """tiles: dict(icon, value, label, href, src, live)"""
    rows = (len(tiles) + cols - 1) // cols
    out = root_open(rail)
    out += ('  <main style="flex: 1; min-width: 0; box-sizing: border-box; padding: 16px; display: flex; flex-direction: column; gap: 12px">\n')
    out += header(title, back)
    out += ('    <div style="flex: 1; min-height: 0; display: grid; grid-template-columns: repeat(%d, minmax(0, 1fr)); '
            'grid-auto-rows: auto; align-content: center; gap: 12px">\n') % cols
    for t in tiles:
        big = t.get('value')
        name_html = '<span style="font-size: 20px; line-height: 24px; font-weight: 800; letter-spacing: -0.02em; white-space: nowrap; overflow: hidden; text-overflow: ellipsis">%s</span>' % t['label']
        cap_style = 'color: #98a1ac; font-variant-numeric: tabular-nums'
        subline = big or t.get('sub', '')
        icon = t.get('icon_html') or svg(t['icon'], 36, '#2dd4bf', '1.8')
        icon = '<span style="height: 44px; display: flex; align-items: center">%s</span>' % icon
        out += ('      <a href="%s" style="border-radius: 24px; background: #15181d; color: #f4f6f8; padding: 16px; box-sizing: border-box; '
                'display: flex; flex-direction: column; justify-content: space-between; min-width: 0; aspect-ratio: 1 / 1">%s'
                '<span style="display: flex; flex-direction: column; gap: 2px; min-width: 0">%s'
                '<span style="height: 36px; font-size: 14px; line-height: 18px; %s; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden">%s</span></span></a>\n') % (
            t['href'], icon, name_html, cap_style, subline)
        note_src(title, t['label'], t.get('src'))
    out += '    </div>\n  </main>\n</div>\n'
    write(name, title, out, 'class Component extends DCLogic {\n  renderVals() {\n    return {};\n  }\n}')

# --- generic form screen -----------------------------------------------------
# row kinds: section, link, toggle, slider, seg, stepper, action, info
FORM_JS = r'''
class Component extends DCLogic {
  constructor(props) {
    super(props);
    var st = {};
    ROWS.forEach(function (r) { if (r.v !== undefined) st[r.id] = r.v; });
    st._drag = null;
    this.state = st;
  }
  fromPointer(e, r) {
    var b = e.currentTarget.getBoundingClientRect();
    var f = Math.max(0, Math.min(1, (b.bottom - e.clientY) / b.height));
    var v = r.min + f * (r.max - r.min);
    return Math.round(v / r.step) * r.step;
  }
  renderVals() {
    var self = this, s = this.state;
    function fmt(r, v) { if (r.fmt === 'off0' && v === 0) return 'Выкл.'; if (v >= 10000) return (v / 1000).toFixed(1).replace('.0', '') + 'k' + (r.unit || ''); return (r.dec ? v.toFixed(r.dec) : String(v)).replace('-', '−') + (r.unit || ''); }
    var rows = ROWS.map(function (r) {
      var o = { kind: r.kind, title: r.t || '', sub: r.sub || '', href: r.href || '#',
        isSection: r.kind === 'section', isLink: r.kind === 'link', isToggle: r.kind === 'toggle', isSlider: r.kind === 'slider',
        isSeg: r.kind === 'seg', isStepper: r.kind === 'stepper', isFine: r.kind === 'fine', isPrimary: r.kind === 'action' && r.tone === 'accent', isSecondary: r.kind === 'action' && r.tone !== 'accent', isInfo: r.kind === 'info', actInk: r.tone === 'danger' ? '#fb7185' : '#2dd4bf',
        value: r.value || '', hasIcon: !!r.ic, p0: (r.ic || [])[0] || '', p1: (r.ic || [])[1] || '', p2: (r.ic || [])[2] || '', p3: (r.ic || [])[3] || '', p4: (r.ic || [])[4] || '', p5: (r.ic || [])[5] || '', tone: r.tone === 'danger' ? '#fb7185' : r.tone === 'accent' ? '#2dd4bf' : '#f4f6f8',
        actBg: r.tone === 'accent' ? '#2dd4bf' : '#1f242b', actFg: r.tone === 'accent' ? '#042f2a' : (r.tone === 'danger' ? '#fb7185' : '#f4f6f8') };
      if (r.kind === 'toggle') {
        var on = !!s[r.id];
        o.aria = on ? 'true' : 'false'; o.track = on ? '#2dd4bf' : '#3a414a'; o.knob = on ? '30px' : '4px';
        o.flip = function () { var n = {}; n[r.id] = !on; self.setState(n); };
      }
      if (r.kind === 'slider') {
        var v = s[r.id];
        o.label = fmt(r, v); o.pct = ((v - r.min) / (r.max - r.min) * 100).toFixed(1) + '%'; o.min = r.min; o.max = r.max; o.now = v;
        o.down = function (e) { if (e.currentTarget.setPointerCapture) e.currentTarget.setPointerCapture(e.pointerId); var n = { _drag: r.id }; n[r.id] = self.fromPointer(e, r); self.setState(n); };
        o.move = function (e) { if (self.state._drag !== r.id) return; var n = {}; n[r.id] = self.fromPointer(e, r); if (n[r.id] !== self.state[r.id]) self.setState(n); };
        o.up = function () { if (self.state._drag) self.setState({ _drag: null }); };
        var big = r.bstep || Math.max(r.step, (r.max - r.min) / 10);
        o.dec = function () { var n = {}; n[r.id] = Math.max(r.min, +(self.state[r.id] - big).toFixed(4)); self.setState(n); };
        o.inc = function () { var n = {}; n[r.id] = Math.min(r.max, +(self.state[r.id] + big).toFixed(4)); self.setState(n); };
        o.decAria = r.t + ': меньше на ' + big; o.incAria = r.t + ': больше на ' + big;
      }
      if (r.kind === 'stepper') {
        var w = s[r.id];
        o.label = fmt(r, w);
        o.dec = function () { var n = {}; n[r.id] = Math.max(r.min, +(w - r.step).toFixed(4)); self.setState(n); };
        o.inc = function () { var n = {}; n[r.id] = Math.min(r.max, +(w + r.step).toFixed(4)); self.setState(n); };
        o.edit = function () { self.setState({ _pad: r.id, _entry: '' }); };
        o.editAria = r.t + ': ' + fmt(r, w) + ', ввести число';
      }
      if (r.kind === 'fine') {
        var fv = s[r.id], big = r.steps[0], small = r.steps[1];
        var setf = function (x) { var n = {}; n[r.id] = Math.min(r.max, Math.max(r.min, +x.toFixed(4))); self.setState(n); };
        o.label = fmt(r, fv); o.valInk = fv !== r.def ? '#5eead4' : '#f4f6f8'; o.ring = fv !== r.def ? 'rgba(45, 212, 191, 0.4)' : 'transparent';
        o.sub = fv !== r.def ? 'было ' + fmt(r, r.def) : (r.sub || '');
        o.decBig = function () { setf(fv - big); }; o.decSmall = function () { setf(fv - small); };
        o.incSmall = function () { setf(fv + small); }; o.incBig = function () { setf(fv + big); };
        o.decBigLabel = '−' + big; o.decSmallLabel = '−' + small; o.incSmallLabel = '+' + small; o.incBigLabel = '+' + big;
        o.decBigAria = r.t + ' минус ' + big; o.decSmallAria = r.t + ' минус ' + small; o.incSmallAria = r.t + ' плюс ' + small; o.incBigAria = r.t + ' плюс ' + big;
        o.edit = function () { self.setState({ _pad: r.id, _entry: '' }); };
        o.editAria = r.t + ': ' + fmt(r, fv) + ', ввести число';
      }
      if (r.kind === 'seg') {
        o.opts = r.opts.map(function (lab, i) {
          var sel = s[r.id] === i;
          return { label: lab, checked: sel ? 'true' : 'false', bg: sel ? '#2dd4bf' : '#1f242b', fg: sel ? '#042f2a' : '#f4f6f8',
            pick: function () { var n = {}; n[r.id] = i; self.setState(n); } };
        });
      }
      return o;
    });
    var pr = null; ROWS.forEach(function (r) { if (r.id === s._pad) pr = r; });
    var e = s._entry || '';
    var num = e === '' || e === '-' ? NaN : parseFloat(e.replace(',', '.'));
    var ok = !!pr && !isNaN(num) && num >= pr.min && num <= pr.max;
    var keys = (pr && pr.min < 0) ? ['1', '2', '3', '4', '5', '6', '7', '8', '9', '±', '0', ','] : ['1', '2', '3', '4', '5', '6', '7', '8', '9', ',', '0', 'C'];
    var bars = rows.filter(function (o) { return o.isSlider; });
    return { rows: rows.filter(function (o) { return !o.isSlider; }), bars: bars,
      padOpen: !!pr, padTitle: pr ? pr.t + (pr.unit ? ', ' + pr.unit.trim() : '') : '',
      entryLabel: e === '' ? '—' : e, entryInk: e === '' ? '#66707b' : '#f4f6f8',
      padHint: pr ? 'от ' + pr.min + ' до ' + pr.max : '', hintInk: (pr && !isNaN(num) && !ok) ? '#fb7185' : '#98a1ac',
      okBg: ok ? '#2dd4bf' : '#1f242b', okFg: ok ? '#042f2a' : '#66707b',
      padOk: function () { if (ok) { var n = { _pad: null }; n[pr.id] = num; self.setState(n); } },
      padCancel: function () { self.setState({ _pad: null }); },
      backspace: function () { self.setState({ _entry: (self.state._entry || '').slice(0, -1) }); },
      keys: keys.map(function (k) {
        return { label: k, aria: k === 'C' ? 'Очистить' : k === '±' ? 'Сменить знак' : k, bg: (k === 'C' || k === '±') ? '#1f242b' : '#252a31',
          press: function () { var t = self.state._entry || ''; if (k === 'C') t = ''; else if (k === '±') t = t.charAt(0) === '-' ? t.slice(1) : '-' + t; else if (k === ',') { if (t.indexOf(',') < 0) t = (t === '' ? '0' : t) + ','; } else if (t.replace('-', '').length < 7) t = t + k; self.setState({ _entry: t }); } };
      }) };
  }
}
'''

PAD_HTML = """  <sc-if value="{{padOpen}}" hint-placeholder-val="{{ false }}">
    <div role="dialog" aria-modal="true" aria-label="Ввести число" style="position: absolute; inset: 0; background: rgba(5, 6, 8, 0.78); display: flex; align-items: center; justify-content: center">
      <div style="width: 680px; height: 420px; box-sizing: border-box; border-radius: 28px; background: #15181d; padding: 20px; display: grid; grid-template-columns: 240px 1fr; gap: 20px">
        <div style="display: flex; flex-direction: column; gap: 8px">
          <span style="font-size: 15px; font-weight: 600; color: #98a1ac">{{padTitle}}</span>
          <div style="height: 96px; border-radius: 20px; background: #0b0d10; display: flex; align-items: center; gap: 6px; padding: 0 8px 0 16px">
            <span aria-live="polite" style="flex: 1; text-align: center; font-size: 40px; font-weight: 800; font-variant-numeric: tabular-nums; color: {{entryInk}}">{{entryLabel}}</span>
            <button type="button" onClick="{{backspace}}" aria-label="Стереть" style="width: 56px; height: 64px; border: 0; border-radius: 16px; background: #1f242b; color: #f4f6f8; font: 600 22px Inter, system-ui, sans-serif; cursor: pointer">⌫</button>
          </div>
          <span style="font-size: 14px; color: {{hintInk}}; text-align: center">{{padHint}}</span>
          <div style="flex: 1"></div>
          <button type="button" onClick="{{padOk}}" style="height: 80px; border: 0; border-radius: 22px; background: {{okBg}}; color: {{okFg}}; font: 800 19px Inter, system-ui, sans-serif; cursor: pointer">Готово</button>
          <button type="button" onClick="{{padCancel}}" style="height: 64px; border: 0; border-radius: 20px; background: #1f242b; color: #f4f6f8; font: 600 16px Inter, system-ui, sans-serif; cursor: pointer">Отмена</button>
        </div>
        <div style="display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); grid-template-rows: repeat(4, minmax(0, 1fr)); gap: 8px">
          <sc-for list="{{keys}}" as="k" hint-placeholder-count="12"><button type="button" onClick="{{k.press}}" aria-label="{{k.aria}}" style="border: 0; border-radius: 18px; background: {{k.bg}}; color: #f4f6f8; font: 700 28px Inter, system-ui, sans-serif; cursor: pointer">{{k.label}}</button></sc-for>
        </div>
      </div>
    </div>
  </sc-if>
"""

BARS_HTML = '''    <div style="flex: 1; min-height: 0; display: flex; gap: 16px">
    <div style="flex: none; display: flex; gap: 10px; padding-bottom: 16px">
      <sc-for list="{{bars}}" as="b" hint-placeholder-count="2">
        <div style="width: 108px; display: flex; flex-direction: column; gap: 6px">
          <span style="font-size: 14px; font-weight: 600; color: #98a1ac; text-align: center; white-space: nowrap; overflow: hidden; text-overflow: ellipsis">{{b.title}}</span>
          <span aria-live="polite" style="font-size: 24px; font-weight: 800; text-align: center; font-variant-numeric: tabular-nums">{{b.label}}</span>
          <div role="slider" aria-label="{{b.title}}" aria-valuemin="{{b.min}}" aria-valuemax="{{b.max}}" aria-valuenow="{{b.now}}" onPointerDown="{{b.down}}" onPointerMove="{{b.move}}" onPointerUp="{{b.up}}" onPointerCancel="{{b.up}}" style="flex: 1; min-height: 120px; position: relative; border-radius: 24px; background: #15181d; overflow: hidden; touch-action: none; cursor: grab">
            <span aria-hidden="true" style="position: absolute; left: 0; right: 0; bottom: 0; height: {{b.pct}}; background: linear-gradient(180deg, #5eead4, #2dd4bf)"></span>
          </div>
          <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 6px">
            <button type="button" onClick="{{b.dec}}" aria-label="{{b.decAria}}" style="height: 56px; border: 0; border-radius: 16px; background: #1f242b; color: #f4f6f8; font: 500 26px Inter, system-ui, sans-serif; cursor: pointer">−</button>
            <button type="button" onClick="{{b.inc}}" aria-label="{{b.incAria}}" style="height: 56px; border: 0; border-radius: 16px; background: #1f242b; color: #f4f6f8; font: 500 26px Inter, system-ui, sans-serif; cursor: pointer">+</button>
          </div>
        </div>
      </sc-for>
    </div>
'''

def form(name, title, rail, back, rows, right='', intro=''):
    out = root_open(rail)
    out += ('  <main style="flex: 1; min-width: 0; box-sizing: border-box; padding: 16px 16px 0 16px; display: flex; flex-direction: column; gap: 10px">\n')
    out += header(title, back, right)
    if intro:
        out += '    <p style="margin: 0; font-size: 15px; line-height: 1.45; color: #98a1ac">%s</p>\n' % intro
    has_bars = any(r.get('kind') == 'slider' for r in rows)
    if has_bars:
        out += BARS_HTML
    out += '''    <div class="scroll" style="flex: 1; min-height: 0; min-width: 0; display: flex; flex-direction: column; gap: 8px; padding-bottom: 16px">
      <sc-for list="{{rows}}" as="r" hint-placeholder-count="6">
        <sc-if value="{{r.isSection}}" hint-placeholder-val="{{ false }}"><span style="margin: 8px 4px 0; font-size: 13px; font-weight: 700; letter-spacing: 0.04em; color: #66707b">{{r.title}}</span></sc-if>
        <sc-if value="{{r.isLink}}" hint-placeholder-val="{{ true }}">
          <a href="{{r.href}}" style="min-height: 72px; flex: none; border-radius: 20px; background: #15181d; color: #f4f6f8; display: flex; align-items: center; gap: 14px; padding: 0 18px">
            <sc-if value="{{r.hasIcon}}" hint-placeholder-val="{{ false }}"><span aria-hidden="true" style="width: 44px; height: 44px; flex: none; border-radius: 14px; background: #1f242b; color: #2dd4bf; display: flex; align-items: center; justify-content: center"><svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="{{r.p0}}"></path><path d="{{r.p1}}"></path><path d="{{r.p2}}"></path><path d="{{r.p3}}"></path><path d="{{r.p4}}"></path><path d="{{r.p5}}"></path></svg></span></sc-if>
            <span style="flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px"><span style="font-size: 18px; font-weight: 700">{{r.title}}</span><span style="font-size: 14px; color: #98a1ac">{{r.sub}}</span></span>
            <span style="font-size: 16px; color: #c7cdd4; white-space: nowrap">{{r.value}}</span>
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="#66707b" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m9 6 6 6-6 6"></path></svg>
          </a>
        </sc-if>
        <sc-if value="{{r.isInfo}}" hint-placeholder-val="{{ false }}">
          <div style="min-height: 64px; flex: none; border-radius: 20px; background: #15181d; display: flex; align-items: center; gap: 14px; padding: 0 18px">
            <span style="flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px"><span style="font-size: 17px; font-weight: 600">{{r.title}}</span><span style="font-size: 14px; color: #98a1ac">{{r.sub}}</span></span>
            <span style="font-size: 17px; font-weight: 700; color: {{r.tone}}; font-variant-numeric: tabular-nums; white-space: nowrap">{{r.value}}</span>
          </div>
        </sc-if>
        <sc-if value="{{r.isToggle}}" hint-placeholder-val="{{ false }}">
          <button type="button" role="switch" aria-checked="{{r.aria}}" onClick="{{r.flip}}" style="min-height: 72px; flex: none; border: 0; border-radius: 20px; background: #15181d; color: #f4f6f8; padding: 0 18px; display: flex; align-items: center; gap: 14px; font-family: Inter, system-ui, sans-serif; text-align: left; cursor: pointer">
            <span style="flex: 1; display: flex; flex-direction: column; gap: 2px"><span style="font-size: 18px; font-weight: 700">{{r.title}}</span><span style="font-size: 14px; color: #98a1ac">{{r.sub}}</span></span>
            <span aria-hidden="true" style="width: 60px; height: 34px; flex: none; border-radius: 17px; background: {{r.track}}; position: relative"><span style="position: absolute; top: 4px; left: {{r.knob}}; width: 26px; height: 26px; border-radius: 13px; background: #ffffff"></span></span>
          </button>
        </sc-if>
        <sc-if value="{{r.isSlider}}" hint-placeholder-val="{{ false }}">
          <div style="min-height: 84px; flex: none; border-radius: 20px; background: #15181d; padding: 0 12px 0 18px; display: flex; align-items: center; gap: 16px">
            <span style="width: 170px; flex: none; display: flex; flex-direction: column; gap: 2px"><span style="font-size: 18px; font-weight: 700">{{r.title}}</span><span style="font-size: 14px; color: #98a1ac">{{r.sub}}</span></span>
            <div role="slider" aria-label="{{r.title}}" aria-valuemin="{{r.min}}" aria-valuemax="{{r.max}}" aria-valuenow="{{r.now}}" onPointerDown="{{r.down}}" onPointerMove="{{r.move}}" onPointerUp="{{r.up}}" onPointerCancel="{{r.up}}" style="flex: 1; height: 60px; border-radius: 18px; background: #1f242b; position: relative; overflow: hidden; touch-action: none; cursor: grab">
              <span aria-hidden="true" style="position: absolute; left: 0; top: 0; bottom: 0; width: {{r.pct}}; background: linear-gradient(90deg, #2dd4bf, #5eead4)"></span>
            </div>
            <span style="width: 86px; flex: none; text-align: right; font-size: 20px; font-weight: 800; font-variant-numeric: tabular-nums">{{r.label}}</span>
          </div>
        </sc-if>
        <sc-if value="{{r.isFine}}" hint-placeholder-val="{{ false }}">
          <div style="min-height: 92px; flex: none; border-radius: 20px; background: #15181d; box-shadow: inset 0 0 0 2px {{r.ring}}; padding: 0 10px 0 18px; display: flex; align-items: center; gap: 8px">
            <span style="flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px"><span style="font-size: 18px; font-weight: 700">{{r.title}}</span><span style="font-size: 14px; color: #98a1ac">{{r.sub}}</span></span>
            <button type="button" onClick="{{r.decBig}}" aria-label="{{r.decBigAria}}" style="width: 64px; height: 64px; border: 0; border-radius: 16px; background: #1f242b; color: #f4f6f8; font: 700 16px Inter, system-ui, sans-serif; cursor: pointer">{{r.decBigLabel}}</button>
            <button type="button" onClick="{{r.decSmall}}" aria-label="{{r.decSmallAria}}" style="width: 64px; height: 64px; border: 0; border-radius: 16px; background: #252a31; color: #f4f6f8; font: 700 16px Inter, system-ui, sans-serif; cursor: pointer">{{r.decSmallLabel}}</button>
            <button type="button" onClick="{{r.edit}}" aria-label="{{r.editAria}}" style="width: 104px; height: 64px; border: 0; border-radius: 16px; background: transparent; color: {{r.valInk}}; font: 800 26px Inter, system-ui, sans-serif; font-variant-numeric: tabular-nums; white-space: nowrap; cursor: pointer">{{r.label}}</button>
            <button type="button" onClick="{{r.incSmall}}" aria-label="{{r.incSmallAria}}" style="width: 64px; height: 64px; border: 0; border-radius: 16px; background: #252a31; color: #f4f6f8; font: 700 16px Inter, system-ui, sans-serif; cursor: pointer">{{r.incSmallLabel}}</button>
            <button type="button" onClick="{{r.incBig}}" aria-label="{{r.incBigAria}}" style="width: 64px; height: 64px; border: 0; border-radius: 16px; background: #1f242b; color: #f4f6f8; font: 700 16px Inter, system-ui, sans-serif; cursor: pointer">{{r.incBigLabel}}</button>
          </div>
        </sc-if>
        <sc-if value="{{r.isStepper}}" hint-placeholder-val="{{ false }}">
          <div style="min-height: 84px; flex: none; border-radius: 20px; background: #15181d; padding: 0 10px 0 18px; display: flex; align-items: center; gap: 10px">
            <span style="flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px"><span style="font-size: 18px; font-weight: 700">{{r.title}}</span><span style="font-size: 14px; color: #98a1ac">{{r.sub}}</span></span>
            <button type="button" onClick="{{r.dec}}" aria-label="Меньше" style="width: 64px; height: 64px; border: 0; border-radius: 18px; background: #1f242b; color: #f4f6f8; font: 500 30px Inter, system-ui, sans-serif; cursor: pointer">−</button>
            <button type="button" onClick="{{r.edit}}" aria-label="{{r.editAria}}" style="width: 120px; height: 64px; border: 0; border-radius: 18px; background: transparent; color: #f4f6f8; font: 800 22px Inter, system-ui, sans-serif; font-variant-numeric: tabular-nums; white-space: nowrap; cursor: pointer">{{r.label}}</button>
            <button type="button" onClick="{{r.inc}}" aria-label="Больше" style="width: 64px; height: 64px; border: 0; border-radius: 18px; background: #1f242b; color: #f4f6f8; font: 500 30px Inter, system-ui, sans-serif; cursor: pointer">+</button>
          </div>
        </sc-if>
        <sc-if value="{{r.isSeg}}" hint-placeholder-val="{{ false }}">
          <div style="min-height: 84px; flex: none; border-radius: 20px; background: #15181d; padding: 10px 10px 10px 18px; box-sizing: border-box; display: flex; flex-wrap: wrap; align-items: center; gap: 10px 14px">
            <span style="flex: 1 1 150px; display: flex; flex-direction: column; gap: 2px"><span style="font-size: 18px; font-weight: 700">{{r.title}}</span><span style="font-size: 14px; color: #98a1ac">{{r.sub}}</span></span>
            <div role="radiogroup" aria-label="{{r.title}}" style="flex: 1 1 280px; display: flex; gap: 6px">
              <sc-for list="{{r.opts}}" as="o" hint-placeholder-count="3">
                <button type="button" role="radio" aria-checked="{{o.checked}}" onClick="{{o.pick}}" style="flex: 1; height: 64px; border: 0; border-radius: 16px; background: {{o.bg}}; color: {{o.fg}}; font: 700 15px Inter, system-ui, sans-serif; cursor: pointer">{{o.label}}</button>
              </sc-for>
            </div>
          </div>
        </sc-if>
        <sc-if value="{{r.isPrimary}}" hint-placeholder-val="{{ false }}">
          <a href="{{r.href}}" style="min-height: 76px; flex: none; border-radius: 20px; background: #2dd4bf; color: #042f2a; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 2px; padding: 8px 18px; box-sizing: border-box; text-align: center"><span style="font-size: 18px; font-weight: 800">{{r.title}}</span><span style="font-size: 13px; opacity: 0.75">{{r.sub}}</span></a>
        </sc-if>
        <sc-if value="{{r.isSecondary}}" hint-placeholder-val="{{ false }}">
          <a href="{{r.href}}" style="min-height: 64px; flex: none; border-radius: 20px; background: #15181d; color: {{r.actInk}}; display: flex; flex-direction: column; justify-content: center; gap: 2px; padding: 10px 18px; box-sizing: border-box">
            <span style="font-size: 17px; font-weight: 700">{{r.title}}</span><span style="font-size: 14px; color: #98a1ac">{{r.sub}}</span>
          </a>
        </sc-if>
      </sc-for>
    </div>
''' + ('    </div>\n' if has_bars else '') + '''  </main>
''' + PAD_HTML + '''</div>
'''
    clean = []
    for i, r in enumerate(rows):
        r = dict(r)
        r.setdefault('id', 'r%d' % i)
        note_src(title, r.get('t', ''), r.pop('src', None))
        if r.get('href', '').startswith('@'):
            r['href'] = name + '.dc.html'
        clean.append(r)
    js = 'var ROWS = ' + json.dumps(clean, ensure_ascii=False, indent=1) + ';\n' + FORM_JS
    write(name, title, out, js)

# =============================================================================
#                                  SCREENS
# =============================================================================
def L(t, sub, href, value='', src=None, icon=None):
    d = dict(kind='link', t=t, sub=sub, href=href + '.dc.html' if href and not href.endswith('.html') and href != '@' else href, value=value, src=src)
    if icon: d['ic'] = ICONS[icon]
    return d
def I(t, value, sub='', src=None, tone=None):
    d = dict(kind='info', t=t, value=value, sub=sub, src=src)
    if tone: d['tone'] = tone
    return d
def T(t, sub, v, src=None):
    return dict(kind='toggle', t=t, sub=sub, v=v, src=src)
def SL(t, sub, v, mn, mx, step, unit='', src=None, fmt=None, dec=0):
    d = dict(kind='slider', t=t, sub=sub, v=v, min=mn, max=mx, step=step, unit=unit, src=src, dec=dec)
    if fmt: d['fmt'] = fmt
    return d
def ST(t, sub, v, mn, mx, step, unit='', dec=0, src=None):
    return dict(kind='stepper', t=t, sub=sub, v=v, min=mn, max=mx, step=step, unit=unit, dec=dec, src=src)
def FS(t, sub, v, mn, mx, steps, unit='%', src=None, dec=0):
    d = dict(kind='fine', t=t, sub=sub, v=v, min=mn, max=mx, steps=steps, unit=unit, src=src, dec=dec); d['def'] = v; return d
    return dict(kind='fine', t=t, sub=sub, v=v, min=mn, max=mx, steps=steps, unit=unit, src=src)
def SG(t, sub, opts, v, src=None):
    return dict(kind='seg', t=t, sub=sub, opts=opts, v=v, src=src)
def A(t, sub='', href='@', tone=None):
    d = dict(kind='action', t=t, sub=sub, href=href if href == '@' or href.endswith('.html') else href + '.dc.html')
    if tone: d['tone'] = tone
    return d
def S(t):
    return dict(kind='section', t=t)

def two_gauges():
    """Two live mini arcs: nozzle (heating) and bed (off), number inside, name under."""
    def one(cur, tgt, mx, name, hot):
        arc = 226.0  # 270 deg of r=48 circle
        cur_len = arc * min(1, cur / mx)
        tf = min(1, tgt / mx)
        import math
        ang = math.radians(135 + tf * 270)
        tx, ty = 60 + 48 * math.cos(ang), 60 + 48 * math.sin(ang)
        stroke = 'url(#hg)' if hot else '#4a525c'
        dot = ('<circle cx="%.1f" cy="%.1f" r="9" fill="#f4f6f8"></circle>' % (tx, ty)) if tgt else ''
        return ('<span style="display: flex; flex-direction: column; align-items: center; gap: 2px">'
                '<span style="position: relative; width: 44px; height: 44px; display: block">'
                '<svg width="44" height="44" viewBox="0 0 120 120" aria-hidden="true">'
                '<defs><linearGradient id="hg" x1="0" y1="1" x2="1" y2="0"><stop offset="0" stop-color="#fbbf24"></stop><stop offset="1" stop-color="#f97316"></stop></linearGradient></defs>'
                '<circle cx="60" cy="60" r="48" fill="none" stroke="#232831" stroke-width="12" stroke-linecap="round" stroke-dasharray="226 302" transform="rotate(135 60 60)"></circle>'
                '<circle cx="60" cy="60" r="48" fill="none" stroke="%s" stroke-width="12" stroke-linecap="round" stroke-dasharray="%.0f 302" transform="rotate(135 60 60)"></circle>%s</svg>'
                '<span style="position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; font-size: 12px; font-weight: 800; font-variant-numeric: tabular-nums">%d°</span></span></span>') % (stroke, cur_len, dot, cur)
    return ('<span role="img" aria-label="Сопло 187 градусов, цель 240; стол 23 градуса, выключен" style="display: flex; gap: 8px">'
            + one(187, 240, 350, 'сопло', True) + one(23, 0, 130, 'стол', False) + '</span>')


def build():
    # ---------- hubs ----------
    gauge = lambda cur, tgt, color: (
        '<svg width="48" height="48" viewBox="0 0 120 120" aria-hidden="true"><circle cx="60" cy="60" r="48" fill="none" stroke="#232831" '
        'stroke-width="14" stroke-linecap="round" stroke-dasharray="226 302" transform="rotate(135 60 60)"></circle><circle cx="60" cy="60" '
        'r="48" fill="none" stroke="%s" stroke-width="14" stroke-linecap="round" stroke-dasharray="%d 302" transform="rotate(135 60 60)"></circle></svg>'
        % (color, cur))
    hub('Control', 'Управление', 'control', [
        dict(icon_html=two_gauges(), label='Нагрев', sub='сопло → 240°, стол выкл.', href='Heating.dc.html',
             src='printer.objects: extruder.temperature/target, heater_bed.temperature/target'),
        dict(icon='move', label='Движение', sub='оси, парковка, моторы', href='Motion.dc.html'),
        dict(icon='extruder', label='Экструдер', sub='выдавить, заправка', href='Extruder.dc.html'),
        dict(icon='fan', value='60%', label='Обдув', href='Fan.dc.html', src='fan.speed (part fan); fan_generic/*.speed'),
        dict(icon='bulb', value='80%', label='Свет', href='Light.dc.html', src='led <name>.color_data[0][3] (white channel)'),
        dict(icon='gauge', value='100%', label='Скорость', href='Speed.dc.html',
             src='gcode_move.speed_factor ×100, gcode_move.extrude_factor ×100'),
        dict(icon='zoff', value='−0.12 мм', label='Z-offset', href='ZOffset.dc.html', src='gcode_move.homing_origin[2]'),
        dict(icon='plug', label='Устройства', sub='питание, пины', href='Devices.dc.html'),
    ])
    hub('Tools', 'Инструменты', 'tools', [
        dict(icon='calib', label='Калибровки', sub='стол, щуп, вибрации', href='Calibrate.dc.html'),
        dict(icon='bolt', label='Макросы', sub='12 макросов', href='Macros.dc.html', src='printer.objects.list → gcode_macro *'),
        dict(icon='console', label='Консоль', sub='G-code вручную', href='Console.dc.html'),
        dict(icon='history', label='История', sub='142 печати', href='History.dc.html', src='server.history.totals'),
        dict(icon='limits', label='Лимиты', sub='скорость, ускорение', href='Limits.dc.html'),
        dict(icon='sensor', label='Датчики', sub='обрыв нити, температуры', href='Sensors.dc.html'),
        dict(icon='service', label='Сервис', sub='фильтр, сопло, рельсы', href='Maintenance.dc.html'),
        dict(icon='camera', label='Камера', sub='и таймлапс', href='Camera.dc.html'),
    ])

    # ---------- Control children ----------
    form('Extruder', 'Экструдер', 'control', 'Control.dc.html', [
        I('Сопло', '240° / 240°', 'для выдавливания нужно ≥ 170°', src='extruder.temperature/target; extruder.can_extrude; configfile.settings.extruder.min_extrude_temp'),
        SG('Длина', 'за одно нажатие', ['5 мм', '10 мм', '25 мм', '50 мм', '100 мм'], 2, src='max: configfile.settings.extruder.max_extrude_only_distance'),
        SG('Скорость', 'мм/с', ['2', '5', '10', '15'], 1, src='max: configfile.settings.extruder.max_extrude_only_velocity'),
        A('Выдавить', 'выбранную длину', tone='accent'),
        A('Втянуть', 'обратно в экструдер'),
        S('ФИЛАМЕНТ'),
        L('Заправить / сменить пруток', 'мастер: нагрев, протяжка, проверка цвета', 'FilamentLoad', icon='load'),
        L('Выгрузить пруток', 'нагреет и вытянет', 'FilamentLoad', icon='unload', src='UNLOAD_FILAMENT macro if present, else G1 E−'),
        L('Катушки Spoolman', 'активная: PETG синий · 640 г', 'Spools', icon='spool', src='server.spoolman.get_spool_id + Spoolman API'),
        L('Ретракт прошивки', 'длина 0.8 мм', 'Retraction', icon='swap', src='firmware_retraction.*'),
        L('Сушка филамента на столе', 'стол как сушилка', 'Dryer', icon='dryer'),
    ])
    form('Retraction', 'Ретракт прошивки', 'control', 'Extruder.dc.html', [
        ST('Длина ретракта', 'директ 0.4–2 мм', 0.8, 0, 6, 0.1, ' мм', 1, src='firmware_retraction.retract_length'),
        ST('Скорость ретракта', '', 40, 5, 120, 5, ' мм/с', src='firmware_retraction.retract_speed'),
        ST('Доп. подача', 'после возврата', 0, 0, 2, 0.05, ' мм', 2, src='firmware_retraction.unretract_extra_length'),
        ST('Скорость подачи', '', 30, 5, 120, 5, ' мм/с', src='firmware_retraction.unretract_speed'),
        I('Действует до перезапуска', '', 'насовсем — только в конфиге принтера'),
    ], intro='Работает, только если в слайсере включён ретракт прошивкой.')
    form('Spools', 'Катушки', 'control', 'Extruder.dc.html', [
        L('PETG · синий', 'Polymaker · 640 г · #12', '@', 'активна', src='server.spoolman.get_spool_id → Spoolman /api/v1/spool'),
        L('PLA · белый', 'eSun · 910 г · #9', '@'),
        L('PLA · чёрный', 'eSun · 210 г · #4', '@'),
        L('TPU · красный', 'Overture · 450 г · #15', '@'),
        A('Снять активную катушку', 'расход не будет списываться'),
    ])
    form('Dryer', 'Сушка филамента', 'control', 'Extruder.dc.html', [
        SG('Материал', 'температура стола', ['PLA 50°', 'PETG 65°', 'ABS 80°', 'TPU 55°'], 1),
        ST('Время', '', 4, 1, 12, 1, ' ч'),
        A('Начать сушку', 'стол выключится сам по таймеру', tone='accent'),
    ])
    form('Devices', 'Устройства', 'control', 'Control.dc.html', [
        S('ПИТАНИЕ'),
        T('Розетка камеры', 'умная розетка по Wi-Fi', False, src='machine.device_power.devices ([power] in moonraker.conf: shelly, tasmota, tplink_smartplug, homeassistant, gpio)'),
        S('ВЫХОДЫ'),
        T('Реле подсветки', '', True, src='output_pin <name>.value (pins starting with _ are hidden)'),
    ], intro='')

    # ---------- Tools children ----------
    form('Calibrate', 'Калибровки', 'tools', 'Tools.dc.html', [
        L('Карта стола', 'профиль «auto» · не загружен', 'BedMesh', icon='mesh', src='bed_mesh.profile_name, bed_mesh.mesh_matrix (max−min)'),
        L('Винты стола', 'сколько крутить каждый винт', 'Screws', icon='screw', src='screws_tilt_adjust (only if configured)'),
        L('Z-offset', 'точная подстройка, проверка датчика', 'ProbeZ', icon='probe', src='probe.*, configfile.settings.probe.z_offset'),
        L('Вибрации', 'X: MZV 56.4 Гц · Y: ZV 46.0 Гц', 'InputShaper', icon='wave', src='configfile.settings.input_shaper.shaper_type_x/freq_x/…'),
        L('Ремни', 'сравнить два ремня', 'Belts', icon='belt', src='requires resonance_tester + accel chip'),
        L('Нагрев (PID)', 'сопло и стол', 'PidCal', icon='thermo', src='configfile.settings.extruder.control/pid_kp…'),
        L('Pressure advance', '0.035', 'PressureAdvance', icon='pa', src='extruder.pressure_advance'),
        L('Драйверы TMC', 'ток, режимы', 'Tmc', icon='driver', src='tmc2209 <stepper>.*'),
        L('Тензодатчик', 'обнулить, если врёт касание', 'Calibrate', icon='scale', src='only when gcode_macro LOAD_CELL_TARE exists (AD5M)'),
    ])
    form('ProbeZ', 'Z-offset', 'tools', 'Calibrate.dc.html', [
        I('Сейчас', '−0.12 мм', 'сохранён в конфиге', src='gcode_move.homing_origin[2]; configfile.settings.probe.z_offset'),
        A('Точная подстройка', 'на тестовом слое', 'ZWizard', tone='accent'),
        A('Проверить точность датчика', '10 касаний в одной точке'),
        I('Последняя проверка', 'разброс 0.004 мм', 'норма до 0.01', src='PROBE_ACCURACY gcode response'),
        S('ДАТЧИК'),
        I('Смещение X / Y', '0 / 0 мм', '', src='configfile.settings.probe.x_offset/y_offset'),
        I('Касаний на точку', '1', '', src='configfile.settings.probe.samples'),
    ])
    form('InputShaper', 'Вибрации', 'tools', 'Calibrate.dc.html', [
        I('Ось X', 'MZV · 56.4 Гц', 'из printer.cfg', src='configfile.settings.input_shaper.shaper_type_x/shaper_freq_x'),
        I('Ось Y', 'ZV · 46.0 Гц', 'из printer.cfg', src='configfile.settings.input_shaper.*_y'),
        SG('Что измерить', '', ['Обе оси', 'X', 'Y'], 0),
        A('Измерить', 'не трогайте принтер во время замера', tone='accent'),
        L('Последний замер', 'графики и все варианты сглаживания', 'ShaperResult', 'вчера'),
        A('Проверить датчик', 'шум акселерометра'),
    ])
    form('PidCal', 'Нагрев (PID)', 'tools', 'Calibrate.dc.html', [
        SG('Нагреватель', '', ['Сопло', 'Стол'], 0, src='heaters.available_heaters'),
        ST('Температура', 'как при обычной печати', 220, 40, 350, 5, '°', src='max: configfile.settings.<heater>.max_temp'),
        SG('Обдув во время', 'как при печати', ['Выкл.', '50%', '100%'], 1),
        I('Текущие', 'Kp 22.3 · Ki 1.1 · Kd 114', '', src='configfile.settings.extruder.pid_kp/ki/kd'),
        A('Запустить', 'нагреватель несколько раз включится и выключится', tone='accent'),
    ])
    form('PressureAdvance', 'Pressure advance', 'tools', 'Calibrate.dc.html', [
        ST('Pressure advance', 'сейчас', 0.04, 0, 1, 0.005, '', 3, src='extruder.pressure_advance'),
        ST('Smooth time', '', 0.04, 0, 0.2, 0.005, ' с', 3, src='extruder.smooth_time'),
        A('Применить', 'действует до перезапуска', tone='accent'),
        A('Тестовая башня', 'печать с PA 0→0.1 по высоте, потом ввести высоту лучшего слоя'),
    ])
    form('Tmc', 'Драйверы TMC', 'tools', 'Calibrate.dc.html', [
        I('Мотор X', '0.80 А · тихий режим', 'TMC2209', src='tmc2209 stepper_x.run_current, drv_status'),
        I('Мотор Y', '0.80 А · тихий режим', 'TMC2209'),
        I('Мотор Z', '0.60 А', 'TMC2209'),
        I('Экструдер', '0.50 А', 'TMC2209'),
    ])
    form('MacroRun', 'AUTO_BED_LEVEL', 'tools', 'Macros.dc.html', [
        ST('BED_TEMP', 'по умолчанию 60', 60, 0, 130, 5, '°'),
        ST('EXTRUDER_TEMP', 'по умолчанию 200', 200, 0, 350, 5, '°'),
        SG('PROFILE', '', ['auto', 'default', 'новый'], 0),
        T('В избранное', 'кнопка на «Инструментах»', True),
        A('Запустить', '', tone='accent'),
    ], intro='Нагрев, протирка сопла и карта стола.')
    form('History', 'История', 'tools', 'Tools.dc.html', [
        I('Всего печатей', '770', 'с первого запуска Moonraker', src='server.history.totals'),
        I('Время печати', '999 ч', 'филамент 8.1 км', src='server.history.totals.total_print_time/total_filament_used'),
        S('ПОСЛЕДНИЕ'),
        L('Корпус вентилятора №2', 'вчера · 1 ч 23 мин · PETG', 'FileDetail', '✓', src='server.history.list'),
        L('Benchy', 'вчера · остановлена на 63%', 'FileDetail', '■'),
        L('Держатель катушки v3', '3 дня · 3 ч 05 мин', 'FileDetail', '✓'),
        A('Очистить историю', 'файлы останутся', tone='danger'),
    ])
    form('Sensors', 'Датчики', 'tools', 'Tools.dc.html', [
        S('ФИЛАМЕНТ'),
        T('Датчик обрыва', 'сейчас: пруток есть', True, src='filament_switch_sensor runout_sensor.enabled/filament_detected'),
        S('ДРУГИЕ ДАТЧИКИ'),
        I('Качество воздуха', 'нет данных', 'AD5M Pro · TVOC, CO₂, формальдегид', src='flashforge_tvoc.tvoc/co2/hcho/status (status "unknown" until the sensor reports)'),
        I('Тензодатчик', '−40 г', 'нагрузка на сопло', src='flashforge_loadcell.force_g'),
        I('Температура MCU и хоста', 'нет датчиков', 'на AD5M не настроены', src='temperature_sensor mcu_temp / host_temp — absent on AD5M, row hidden in the app'),
    ])
    # ---------- Settings ----------
    form('Settings', 'Настройки', 'settings', None, [
        L('Сеть', 'Wi-Fi HomeNet_5G · 192.168.1.42', 'Network', icon='wifi', src='host: wpa_supplicant/NetworkManager; machine.system_info.network'),
        L('Экран', 'яркость 70% · сон 10 мин', 'Screen', icon='screen'),
        L('Блокировка и PIN', 'выкл.', 'LockSettings', icon='lock'),
        L('Звук', 'сигнал в конце печати', 'Sound', icon='sound'),
        L('Язык и время', 'Русский · 24 ч · Europe/Moscow', 'LangTime', icon='lang'),
        L('Безопасность', 'подтверждения', 'Safety', icon='shield'),
        L('Принтеры', '1 подключён', 'Printers', icon='printer'),
        L('Обновления', 'Klipper v0.13.0 · Mainsail, Fluidd', 'Updates', icon='update', src='printer.info.software_version; machine.update.status if available'),
        L('Система', 'перезапуск, выключение, о системе', 'System', icon='chip'),
    ])
    form('Network', 'Сеть', 'settings', 'Settings.dc.html', [
        T('Wi-Fi', '', True, src='host wpa_supplicant'),
        I('Подключено', 'HomeNet_5G', '192.168.1.42 · сигнал −54 дБм', src='machine.system_info.network.wlan0'),
        S('ДОСТУПНЫЕ СЕТИ'),
        L('HomeNet_5G', '5 ГГц · WPA2', 'WifiJoin', 'подключено'),
        L('HomeNet', '2.4 ГГц · WPA2', 'WifiJoin'),
        L('Guest', 'открытая', 'WifiJoin'),
        L('Скрытая сеть', 'ввести имя вручную', 'WifiJoin'),
        S('ПРОЧЕЕ'),
        I('Имя хоста', 'ad5m', '', src='machine.system_info.network / hostname'),
        I('Ethernet', 'не подключён', ''),
    ])
    form('WifiJoin', 'HomeNet', 'settings', 'Network.dc.html', [
        I('Пароль', '••••••••', 'нажмите, чтобы ввести — откроется клавиатура'),
        T('Подключаться автоматически', '', True),
        A('Подключиться', '', tone='accent'),
        A('Забыть сеть', '', tone='danger'),
    ])
    form('Screen', 'Экран', 'settings', 'Settings.dc.html', [
        SL('Яркость', 'подсветка экрана', 70, 10, 100, 5, '%', src='AD5M: LCD_ON BACKLIGHT=<pct>; generic: /sys/class/backlight'),
        SG('Гасить через, мин', 'без касаний', ['1', '5', '10', '30', '∞'], 2),
        T('Не гасить во время печати', 'показывать заставку с прогрессом', True),
        SG('Поворот', '', ['0°', '90°', '180°', '270°'], 0),
        A('Калибровка тача', 'коснуться 4 точек'),
    ])
    form('LockSettings', 'Блокировка и PIN', 'settings', 'Settings.dc.html', [
        T('Блокировка во время печати', 'разблокировать — провести пальцем', False),
        T('PIN-код', '4–6 цифр', False),
        SG('Автоблокировка', 'без касаний', ['Нет', '5 мин', '15 мин', '30 мин'], 0),
    ])
    form('Sound', 'Звук', 'settings', 'Settings.dc.html', [
        T('Звуки', 'пищалка принтера', True, src='M300 (AD5M macro) / output_pin beeper'),
        SL('Громкость', '', 60, 0, 100, 10, '%', fmt='off0'),
        T('Мелодия в конце печати', '', True, src='gcode_macro PLAY_MIDI (KlipperMod)'),
        T('Звук касаний', '', False),
        A('Проверить звук', ''),
    ])
    form('LangTime', 'Язык и время', 'settings', 'Settings.dc.html', [
        SG('Язык', '', ['Русский', 'English'], 0),
        SG('Формат времени', '', ['24 ч', '12 ч'], 0),
        L('Часовой пояс', 'для «готово в 15:20»', '@', 'Europe/Moscow', src='host timezone'),
    ])
    form('Safety', 'Безопасность', 'settings', 'Settings.dc.html', [
        T('Удерживать «Стоп» 1 секунду', 'защита от случайного касания; иначе — сразу по нажатию', False),
        T('Если отмена зависла — аварийная остановка', '', True),
        ST('Ждать перед E-stop', '', 15, 5, 60, 5, ' с'),
        T('Подтверждать запуск макросов', '', False),
        SG('Уведомления', 'что показывать поверх экрана', ['Все', 'Важные', 'Ошибки'], 1),
    ])
    form('Printers', 'Принтеры', 'settings', 'Settings.dc.html', [
        L('AD5M', 'этот принтер · подключён', '@', '✓', src='Rivet config: list of Moonraker URLs'),
        A('Добавить принтер', 'адрес Moonraker и порт', tone='accent'),
    ])
    form('Updates', 'Обновления', 'settings', 'Settings.dc.html', [
        S('ВЕРСИИ'),
        I('Klipper', 'v0.13.0-285', 'обновляется образом KlipperMod с USB', src='printer.info.software_version; update_manager has no version for klipper on KlipperMod'),
        I('Moonraker', 'v0.9.3', 'обновляется образом KlipperMod с USB', src='server.info.moonraker_version'),
        I('Rivet', '0.1.0', '', src='Rivet build'),
        S('ВЕБ-ИНТЕРФЕЙСЫ'),
        I('Mainsail', 'v2.19.0', 'актуально', src='machine.update.status.version_info.mainsail (version == remote_version)'),
        I('Fluidd', 'v1.37.7', 'актуально', src='machine.update.status.version_info.fluidd'),
        A('Проверить обновления', ''),
    ])
    form('System', 'Система', 'settings', 'Settings.dc.html', [
        S('ПЕРЕЗАПУСК'),
        A('Перезапустить Klipper', 'перечитает конфиг'),
        A('Перезапустить прошивку', 'после ошибки платы'),
        A('Перезапустить Moonraker', ''),
        A('Перезапустить экран', ''),
        S('ПИТАНИЕ'),
        T('Выключить после печати', 'когда остынет ниже 50°', False, src='AUTO_SHUTDOWN (KlipperMod)'),
        A('Перезагрузить принтер', ''),
        A('Выключить принтер', '', tone='danger'),
        A('Загрузить стоковую прошивку', 'вернуться в прошивку Flashforge до перезагрузки'),
        S('О СИСТЕМЕ'),
        I('Процессор', 'T113-S3 · 2 ядра · 18%', '', src='machine.system_info.cpu_info; machine.proc_stats'),
        I('Память', '54 / 128 МБ', '', src='machine.proc_stats.system_memory'),
        I('Диск', '1.2 / 7.1 ГБ', '', src='server.files.get_directory → disk_usage'),
        SG('Логи', 'уровень', ['Ошибки', 'Инфо', 'Отладка'], 1),
        A('Очистить место', 'старые логи, таймлапсы, кэш миниатюр'),
        A('Сбросить настройки Rivet', 'Klipper не трогает', tone='danger'),
    ])

    # ---------- Files ----------
    form('JobQueue', 'Очередь печати', 'home', 'Files.dc.html', [
        L('1 · Benchy', '52 мин · PLA', 'FileDetail', src='server.job_queue.status'),
        L('2 · Крышка электроники', '41 мин · PLA', 'FileDetail'),
        A('Начать очередь', 'всего 1 ч 33 мин', tone='accent'),
        T('Ждать подтверждения между печатями', 'снять деталь со стола', True),
        A('Очистить очередь', '', tone='danger'),
    ])
    form('FileActions', 'Корпус вентилятора №2', 'home', 'FileDetail.dc.html', [
        A('Добавить в очередь', ''),
        A('Переименовать', ''),
        A('Переместить в папку', ''),
        A('Удалить', '', tone='danger'),
    ])

    # ---------- Printing ----------
    form('PrintTune', 'Подстройка печати', 'home', 'Printing.dc.html', [
        FS('Скорость', 'от слайсера', 100, 10, 300, [10, 1], src='gcode_move.speed_factor'),
        FS('Поток', 'сколько пластика', 100, 50, 150, [5, 1], src='gcode_move.extrude_factor'),
        FS('Z-offset, мм', 'меняется сразу', -0.12, -2, 2, [0.05, 0.01], '', src='gcode_move.homing_origin[2]', dec=2),
        A('Сохранить Z-offset', 'запишется после печати'),
        ST('PA', 'pressure advance', 0.04, 0, 1, 0.005, '', 3, src='extruder.pressure_advance'),
        L('Температуры', 'сопло 240° · стол 80°', 'Heating'),
        L('Вентиляторы', 'обдув 60%', 'Fan'),
        A('Сбросить скорость и поток', 'обратно на 100%'),
    ])
    form('PrintMore', 'Во время печати', 'home', 'Printing.dc.html', [
        L('Пауза на слое', 'следующий или выбранный', 'LayerPause', src='print_stats.info.current_layer/total_layer; SET_PAUSE_AT_LAYER'),
        L('Пропуск объектов', '4 детали', 'Exclude', src='exclude_object.objects'),
        L('Камера', '', 'Camera'),
        T('Свет', '', True, src='led chamber_led'),
        T('Таймлапс', 'пишется', True),
        T('Выключить после печати', '', False),
    ])
    form('LayerPause', 'Пауза на слое', 'home', 'PrintMore.dc.html', [
        I('Сейчас', 'слой 76 из 180', '', src='print_stats.info.current_layer/total_layer'),
        ST('Пауза на слое', 'вставить магнит, сменить цвет', 120, 77, 180, 1, ''),
        A('Поставить паузу', '', tone='accent'),
        A('На следующем слое', ''),
    ], intro='')

    # ---------- more Control ----------
    form('Fan', 'Вентиляторы', 'control', 'Control.dc.html', [
        SL('Обдув', '', 60, 0, 100, 5, '%', src='fan.speed', fmt='off0'),
        SL('Камера', '', 0, 0, 100, 5, '%', src='fan_generic chamber_fan.speed', fmt='off0'),
        SG('Воздух в камере', '', ['Выкл.', 'Фильтр', 'Вытяжка'], 0, src='only when AIR_CIRCULATION_* macros exist'),
        S('АВТОМАТИЧЕСКИЕ'),
        I('Хотэнд', '100%', 'включается сам от 50°', src='heater_fan hotend_fan.speed'),
        I('Драйверы', '0%', 'работает, пока включены моторы', src='controller_fan stepper_driver_fan.speed'),
    ], intro='Слайсер может поменять обдув модели на следующем слое.')
    form('Light', 'Свет', 'control', 'Control.dc.html', [
        T('Подсветка', '', True, src='led chamber_led.color_data'),
        SL('Яркость', '', 80, 0, 100, 5, '%', src='led chamber_led.color_data[0][3]', fmt='off0'),
        T('Включать при старте', '', True),
    ])
    form('Speed', 'Скорость и поток', 'control', 'Control.dc.html', [
        FS('Скорость', 'от слайсера', 100, 10, 300, [10, 1], src='gcode_move.speed_factor'),
        FS('Поток', 'сколько пластика', 100, 50, 150, [5, 1], src='gcode_move.extrude_factor'),
        A('Вернуть 100% / 100%', ''),
        I('Сейчас', '86 мм/с', 'реальная скорость головы', src='motion_report.live_velocity'),
    ], intro='Выше 150% качество и прочность обычно падают.')
    form('MotionAdvanced', 'Движение · ещё', 'control', 'Motion.dc.html', [
        S('ПАРКОВКА'),
        A('Все оси', ''), A('Только XY', ''), A('Только Z', ''),
        S('ПЕРЕЙТИ В ТОЧКУ'),
        ST('X', '−112 … 112', 0, -112, 112, 1, ' мм', src='toolhead.axis_minimum/axis_maximum'),
        ST('Y', '−112 … 112', 0, -112, 112, 1, ' мм'),
        ST('Z', '−10 … 230', 10, -10, 230, 1, ' мм'),
        A('Поехать', '', tone='accent'),
        S('НАСТРОЙКИ ДЖОЙСТИКА'),
        ST('Скорость XY', '', 100, 10, 600, 10, ' мм/с', src='max: toolhead.max_velocity'),
        ST('Скорость Z', '', 10, 1, 25, 1, ' мм/с', src='max: configfile.settings.printer.max_z_velocity'),
        T('Инвертировать X', 'только на экране', False), T('Инвертировать Y', '', False), T('Инвертировать Z', '', False),
        A('Проверить концевики', 'нажаты или нет'),
    ])
    # ---------- printing extras ----------
if __name__ == '__main__':
    build()
    # write sources doc
    doc = sys.argv[1] if len(sys.argv) > 1 else None
    if doc:
        with open(doc, 'w') as f:
            f.write('# Data sources (generated)\n\nEvery value shown on generated screens and where Rivet reads it. '
                    'Produced by `docs/ux/style/gen.py`.\n\n| Screen | Item | Source |\n|---|---|---|\n')
            for sc, lab, src in SOURCES:
                f.write('| %s | %s | `%s` |\n' % (sc, lab.replace('|', '/'), src.replace('|', '/')))
    print('ok', len(SOURCES))
