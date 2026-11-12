"""Hand-drawn screens that need their own layout (print done, runout, Z wizard, heat presets)."""
import gen
from gen import write, root_open, header, svg

EMPTY = 'class Component extends DCLogic {\n  renderVals() {\n    return {};\n  }\n}'

STEP_ROW = """      <a href="{href}" style="min-height: 72px; border-radius: 20px; background: #15181d; color: #f4f6f8; display: flex; align-items: center; gap: 14px; padding: 0 18px">
        <span style="width: 40px; height: 40px; flex: none; border-radius: 20px; background: #1f242b; color: #5eead4; font-size: 17px; font-weight: 800; display: flex; align-items: center; justify-content: center">{n}</span>
        <span style="flex: 1; display: flex; flex-direction: column; gap: 2px"><span style="font-size: 17px; font-weight: 700">{t}</span><span style="font-size: 14px; color: #98a1ac">{d}</span></span>
      </a>
"""

STEPPER = ('<div style="display: flex; align-items: center; gap: 6px">'
           '<button type="button" onClick="{{m.%sDec}}" aria-label="%s меньше" style="width: 64px; height: 64px; border: 0; border-radius: 16px; background: #1f242b; color: #f4f6f8; font: 500 28px Inter, system-ui, sans-serif; cursor: pointer">−</button>'
           '<span style="flex: 1; text-align: center; font-size: 22px; font-weight: 800; font-variant-numeric: tabular-nums">{{m.%s}}°</span>'
           '<button type="button" onClick="{{m.%sInc}}" aria-label="%s больше" style="width: 64px; height: 64px; border: 0; border-radius: 16px; background: #1f242b; color: #f4f6f8; font: 500 28px Inter, system-ui, sans-serif; cursor: pointer">+</button></div>')


def build_more():
    # ---- Print done: hero, not a settings list
    body = root_open('home') + """  <main style="flex: 1; min-width: 0; box-sizing: border-box; padding: 24px 24px 16px; display: grid; grid-template-columns: 250px 1fr; gap: 28px">
    <div aria-hidden="true" style="width: 250px; height: 250px; border-radius: 28px; background: radial-gradient(circle at 50% 42%, #232a36 0%, #15181d 72%); display: flex; align-items: center; justify-content: center; position: relative">
      <svg width="190" height="190" viewBox="0 0 200 200"><ellipse cx="100" cy="176" rx="74" ry="12" fill="#0b0d10" opacity="0.6"></ellipse><polygon points="100,40 170,80 100,120 30,80" fill="#5b7de0"></polygon><polygon points="30,80 100,120 100,168 30,128" fill="#2f4fa8"></polygon><polygon points="100,120 170,80 170,128 100,168" fill="#22397e"></polygon></svg>
      <span style="position: absolute; right: 14px; top: 14px; width: 48px; height: 48px; border-radius: 24px; background: #2dd4bf; color: #042f2a; display: flex; align-items: center; justify-content: center">""" + svg('check', 28, 'currentColor', '2.6') + """</span>
    </div>
    <div style="display: flex; flex-direction: column; gap: 10px; min-width: 0">
      <span style="font-size: 15px; font-weight: 700; color: #5eead4">Готово · 14:02</span>
      <h1 style="margin: 0; font-size: 32px; font-weight: 800; letter-spacing: -0.02em; line-height: 1.15">Корпус вентилятора №2</h1>
      <div style="display: flex; gap: 10px; margin-top: 6px">
        <div style="flex: 1; border-radius: 18px; background: #15181d; padding: 12px 14px; display: flex; flex-direction: column; gap: 2px"><span style="font-size: 13px; color: #98a1ac">Время</span><span style="font-size: 22px; font-weight: 800">1 ч 23 мин</span></div>
        <div style="flex: 1; border-radius: 18px; background: #15181d; padding: 12px 14px; display: flex; flex-direction: column; gap: 2px"><span style="font-size: 13px; color: #98a1ac">Пластик</span><span style="font-size: 22px; font-weight: 800">18 г PETG</span></div>
      </div>
      <span style="font-size: 15px; color: #98a1ac; margin-top: 4px">Стол 61° — PETG легче снять, когда остынет.</span>
      <div style="flex: 1"></div>
      <div style="display: grid; grid-template-columns: 1fr 1.4fr; gap: 10px">
        <a href="Main.dc.html" style="height: 80px; border-radius: 22px; background: #15181d; color: #f4f6f8; font-size: 18px; font-weight: 700; display: flex; align-items: center; justify-content: center">Готово</a>
        <a href="FileDetail.dc.html" style="height: 80px; border-radius: 22px; background: #2dd4bf; color: #042f2a; font-size: 18px; font-weight: 800; display: flex; align-items: center; justify-content: center">Печатать снова</a>
      </div>
    </div>
  </main>
</div>
"""
    write('PrintDone', 'Печать завершена', body, EMPTY)
    gen.note_src('Печать завершена', 'time, filament', 'print_stats.print_duration, filament_used; history job')

    # ---- Runout: numbered steps, resume as the one primary action
    steps = [('1', 'Выгрузить остаток', 'нагреем и вытянем обрывок', 'Runout.dc.html'),
             ('2', 'Заправить новый пруток', 'мастер загрузки', 'FilamentLoad.dc.html'),
             ('3', 'Прочистить сопло', '25 мм, чтобы пошёл чистый цвет', 'Runout.dc.html')]
    rows = ''.join(STEP_ROW.format(href=h, n=n, t=t, d=d) for n, t, d, h in steps)
    body = root_open('home') + """  <main style="flex: 1; min-width: 0; box-sizing: border-box; padding: 16px; display: grid; grid-template-columns: 240px 1fr; gap: 16px">
    <div style="border-radius: 24px; background: rgba(251, 191, 36, 0.08); padding: 20px; display: flex; flex-direction: column; gap: 10px">
      <span aria-hidden="true" style="width: 52px; height: 52px; border-radius: 18px; background: rgba(251, 191, 36, 0.16); color: #fbbf24; display: flex; align-items: center; justify-content: center">""" + svg('pause', 28) + """</span>
      <h1 style="margin: 4px 0 0; font-size: 24px; font-weight: 800; line-height: 1.2">Закончился филамент</h1>
      <span style="font-size: 15px; line-height: 1.45; color: #c7cdd4">Печать на паузе на слое 76 из 180. Сопло держит температуру.</span>
      <div style="flex: 1"></div>
      <a href="ConfirmStop.dc.html" style="height: 64px; border-radius: 18px; background: rgba(251, 75, 94, 0.14); color: #fb7185; font-size: 16px; font-weight: 700; display: flex; align-items: center; justify-content: center">Отменить печать</a>
    </div>
    <div style="display: flex; flex-direction: column; gap: 8px">
""" + rows + """      <div style="flex: 1"></div>
      <a href="Printing.dc.html" style="height: 80px; border-radius: 22px; background: #2dd4bf; color: #042f2a; font-size: 19px; font-weight: 800; display: flex; align-items: center; justify-content: center">Продолжить печать</a>
    </div>
  </main>
</div>
"""
    write('Runout', 'Закончился филамент', body, EMPTY)
    gen.note_src('Закончился филамент', 'state', 'filament_switch_sensor.filament_detected=false; print_stats.state=paused; print_stats.info.current_layer')

    # ---- Z-offset paper wizard: height, step, two big buttons
    body = root_open('tools') + """  <main style="flex: 1; min-width: 0; box-sizing: border-box; padding: 16px; display: flex; flex-direction: column; gap: 12px">
""" + header('Точная подстройка Z', 'ProbeZ.dc.html') + """    <div style="flex: 1; min-height: 0; display: grid; grid-template-columns: 1fr 260px; gap: 16px">
      <div style="display: flex; flex-direction: column; gap: 12px; min-width: 0">
        <p style="margin: 0; font-size: 17px; line-height: 1.5; color: #c7cdd4">Принтер печатает тестовый слой. Линии рвутся и тонкие — опустите сопло. Наплывы и волны — поднимите.</p>
        <div style="border-radius: 22px; background: #15181d; padding: 14px 18px; display: flex; align-items: center; gap: 10px"><span style="font-size: 15px; color: #98a1ac">Z-offset, мм</span><span style="flex: 1"></span><span style="font-size: 40px; font-weight: 800; font-variant-numeric: tabular-nums">{{zLabel}}</span></div>
        <span style="font-size: 14px; font-weight: 600; color: #98a1ac">Шаг, мм</span>
        <div role="radiogroup" aria-label="Шаг" style="display: flex; gap: 6px">
          <sc-for list="{{steps}}" as="o" hint-placeholder-count="5"><button type="button" role="radio" aria-checked="{{o.checked}}" onClick="{{o.pick}}" style="flex: 1; height: 64px; border: 0; border-radius: 16px; background: {{o.bg}}; color: {{o.fg}}; font: 700 16px Inter, system-ui, sans-serif; cursor: pointer">{{o.label}}</button></sc-for>
        </div>
        <div style="flex: 1"></div>
        <div style="display: grid; grid-template-columns: 1fr 1.4fr; gap: 10px">
          <a href="ProbeZ.dc.html" style="height: 76px; border-radius: 20px; background: rgba(251, 75, 94, 0.14); color: #fb7185; font-size: 17px; font-weight: 700; display: flex; align-items: center; justify-content: center">Прервать тест</a>
          <a href="ProbeZ.dc.html" style="height: 76px; border-radius: 20px; background: #2dd4bf; color: #042f2a; font-size: 17px; font-weight: 800; display: flex; flex-direction: column; align-items: center; justify-content: center"><span>Сохранить</span><span style="font-size: 12px; font-weight: 600; opacity: 0.75">после теста</span></a>
        </div>
      </div>
      <div style="display: grid; grid-template-rows: 1fr 1fr; gap: 10px">
        <button type="button" onClick="{{upZ}}" style="border: 0; border-radius: 24px; background: #1f242b; color: #f4f6f8; font-family: Inter, system-ui, sans-serif; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 4px; cursor: pointer"><span style="font-size: 34px; line-height: 1">↑</span><span style="font-size: 16px; font-weight: 700">Поднять</span></button>
        <button type="button" onClick="{{downZ}}" style="border: 0; border-radius: 24px; background: #252a31; color: #f4f6f8; font-family: Inter, system-ui, sans-serif; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 4px; cursor: pointer"><span style="font-size: 34px; line-height: 1">↓</span><span style="font-size: 16px; font-weight: 700">Опустить</span></button>
      </div>
    </div>
  </main>
</div>
"""
    js = """class Component extends DCLogic {
  constructor(props) { super(props); this.state = { z: -0.12, step: 0.01 }; }
  renderVals() {
    var self = this, st = this.state;
    var opts = [0.005, 0.01, 0.05, 0.1, 1];
    return {
      zLabel: st.z.toFixed(3).replace('.', ',').replace('-', '−'),
      upZ: function () { self.setState({ z: +(st.z + st.step).toFixed(3) }); },
      downZ: function () { self.setState({ z: +(st.z - st.step).toFixed(3) }); },
      steps: opts.map(function (v) { var on = v === st.step; return { label: String(v).replace('.', ','), checked: on ? 'true' : 'false', bg: on ? '#2dd4bf' : '#15181d', fg: on ? '#042f2a' : '#f4f6f8', pick: function () { self.setState({ step: v }); } }; })
    };
  }
}"""
    write('ZWizard', 'Точная подстройка Z', body, js)
    gen.note_src('Точная подстройка Z', 'z', 'prints a one-layer test pattern; SET_GCODE_OFFSET Z_ADJUST=±step; save = Z_OFFSET_APPLY_PROBE + SAVE_CONFIG')

    # ---- Heat presets: one card per material, nozzle and bed side by side
    body = root_open('control') + """  <main style="flex: 1; min-width: 0; box-sizing: border-box; padding: 16px; display: flex; flex-direction: column; gap: 10px">
""" + header('Пресеты нагрева', 'Heating.dc.html') + """    <div style="display: grid; grid-template-columns: 90px 1fr 1fr; gap: 10px; padding: 0 22px 0 18px; font-size: 13px; font-weight: 700; letter-spacing: 0.04em; color: #66707b"><span></span><span style="text-align: center">СОПЛО</span><span style="text-align: center">СТОЛ</span></div>
    <div class="scroll" style="flex: 1; min-height: 0; display: flex; flex-direction: column; gap: 8px">
      <sc-for list="{{mats}}" as="m" hint-placeholder-count="4">
        <div style="min-height: 80px; flex: none; border-radius: 20px; background: #15181d; padding: 8px 8px 8px 18px; display: grid; grid-template-columns: 82px 1fr 1fr; gap: 10px; align-items: center">
          <span style="font-size: 18px; font-weight: 800">{{m.name}}</span>
          """ + STEPPER % ('n', 'Сопло', 'n', 'n', 'Сопло') + """
          """ + STEPPER % ('b', 'Стол', 'b', 'b', 'Стол') + """
        </div>
      </sc-for>
      <button type="button" style="min-height: 64px; flex: none; border: 2px dashed #2a3038; border-radius: 20px; background: transparent; color: #98a1ac; font: 700 16px Inter, system-ui, sans-serif; cursor: pointer">+ Добавить материал</button>
    </div>
  </main>
</div>
"""
    js = """class Component extends DCLogic {
  constructor(props) { super(props); this.state = { m: [['PLA', 210, 60], ['PETG', 240, 80], ['ABS', 250, 100], ['TPU', 225, 50]] }; }
  renderVals() {
    var self = this;
    function upd(i, j, d, max) { var m = self.state.m.map(function (r) { return r.slice(); }); m[i][j] = Math.max(0, Math.min(max, m[i][j] + d)); self.setState({ m: m }); }
    return { mats: this.state.m.map(function (r, i) { return { name: r[0], n: r[1], b: r[2],
      nDec: function () { upd(i, 1, -5, 350); }, nInc: function () { upd(i, 1, 5, 350); },
      bDec: function () { upd(i, 2, -5, 130); }, bInc: function () { upd(i, 2, 5, 130); } }; }) };
  }
}"""
    write('HeatPresets', 'Пресеты нагрева', body, js)
    gen.note_src('Пресеты нагрева', 'presets', 'Rivet presets in Moonraker database; max from configfile.settings.<heater>.max_temp')


import math


def _psd(f, peaks):
    """Sum of resonance bumps; peaks = [(freq, height, width)]."""
    return sum(h / (1 + ((f - f0) / w) ** 2) for f0, h, w in peaks) + 0.04


def _poly(fn, x0, y0, w, h, fmax=150, vmax=1.15):
    pts = []
    for i in range(0, 151):
        f = 5 + i * (fmax - 5) / 150.0
        v = min(vmax, fn(f))
        pts.append('%.1f,%.1f' % (x0 + (f - 5) / (fmax - 5) * w, y0 + h - v / vmax * h))
    return ' '.join(pts)


def _axes(x0, y0, w, h, fmax=150):
    out = ''
    for f in (25, 50, 75, 100, 125):
        x = x0 + (f - 5) / (fmax - 5) * w
        out += '<path d="M%.1f %d V%d" stroke="#1f242b" stroke-width="1"></path><text x="%.1f" y="%d" fill="#66707b" font-size="11" text-anchor="middle" font-family="Inter, sans-serif">%d</text>' % (x, y0, y0 + h, x, y0 + h + 14, f)
    out += '<text x="%d" y="%d" fill="#66707b" font-size="11" text-anchor="end" font-family="Inter, sans-serif">Гц</text>' % (x0 + w, y0 + h + 14)
    return out


def build_graphs():
    # ---- Input shaper result: X/Y tabs, PSD + chosen shaper, table of options
    W, H, X0, Y0 = 420, 230, 10, 10
    px = [(54.2, 1.0, 5.5), (98, 0.18, 8)]
    py = [(41.8, 0.85, 4.5), (66, 0.25, 6)]

    def shaped(peaks, fs):
        # smooth attenuation centred on the shaper frequency
        return lambda f: _psd(f, peaks) * (1 - 0.9 * math.exp(-((f - fs) / (0.45 * fs)) ** 2))
    graphs = {}
    table = {
        'x': [['MZV', '54.2', '1%', '7 400', True], ['EI', '62.0', '0%', '6 100', False], ['ZV', '58.6', '4%', '11 200', False], ['2HUMP_EI', '74.4', '0%', '4 900', False], ['3HUMP_EI', '89.2', '0%', '4 300', False]],
        'y': [['MZV', '40.4', '3%', '4 100', False], ['EI', '41.8', '1%', '5 900', True], ['ZV', '44.0', '7%', '6 700', False], ['2HUMP_EI', '55.6', '0%', '2 800', False], ['3HUMP_EI', '66.8', '0%', '2 400', False]],
    }
    for ax, peaks in (('x', px), ('y', py)):
        graphs[ax] = (_poly(lambda f: _psd(f, peaks), X0, Y0, W, H),
                      [_poly(shaped(peaks, float(r[1])), X0, Y0, W, H) for r in table[ax]])
    svg_tpl = ('<svg role="img" aria-label="{{graphAria}}" viewBox="0 0 440 260" style="width: 100%%; height: 100%%">%s'
               '<polyline points="{{raw}}" fill="none" stroke="#98a1ac" stroke-width="2" stroke-dasharray="4 4" stroke-linejoin="round"></polyline>'
               '<polyline points="{{shapedPts}}" fill="none" stroke="#2dd4bf" stroke-width="3" stroke-linejoin="round"></polyline>'
               '<path d="M{{peakX}} 10 V240" stroke="#fbbf24" stroke-width="1.5" stroke-dasharray="3 3"></path></svg>') % _axes(X0, Y0, W, H)
    rows = """          <sc-for list="{{shapers}}" as="t" hint-placeholder-count="5">
            <button type="button" role="radio" aria-checked="{{t.checked}}" onClick="{{t.pick}}" style="height: 44px; flex: none; border: 0; border-radius: 12px; background: {{t.bg}}; box-shadow: inset 0 0 0 2px {{t.ring}}; padding: 0 10px; display: grid; grid-template-columns: 1.7fr 0.9fr 0.8fr 1.1fr; align-items: center; font: 400 13px Inter, system-ui, sans-serif; font-variant-numeric: tabular-nums; color: {{t.fg}}; text-align: left; cursor: pointer">
              <span style="font-weight: 800">{{t.name}}</span><span>{{t.freq}}</span><span>{{t.vib}}</span><span>{{t.acc}}</span>
            </button>
          </sc-for>
"""
    body = root_open('tools') + """  <main style="flex: 1; min-width: 0; box-sizing: border-box; padding: 16px; display: flex; flex-direction: column; gap: 10px">
""" + header('Вибрации · замер', 'InputShaper.dc.html', '<div role="tablist" aria-label="Ось" style="height: 64px; box-sizing: border-box; padding: 5px; border-radius: 20px; background: #15181d; display: flex; gap: 4px"><sc-for list="{{axes}}" as="a" hint-placeholder-count="2"><button type="button" role="tab" aria-selected="{{a.sel}}" onClick="{{a.pick}}" style="width: 76px; border: 0; border-radius: 15px; background: {{a.bg}}; color: {{a.fg}}; font: 700 17px Inter, system-ui, sans-serif; cursor: pointer">{{a.label}}</button></sc-for></div>') + """    <div style="flex: 1; min-height: 0; display: grid; grid-template-columns: 1fr 250px; gap: 12px">
      <div style="border-radius: 20px; background: #15181d; padding: 8px; display: flex; flex-direction: column; min-height: 0">
        <div style="flex: 1; min-height: 0">""" + svg_tpl + """</div>
        <div style="display: flex; gap: 16px; padding: 4px 8px 2px; font-size: 13px; color: #98a1ac"><span><b style="color: #98a1ac">- -</b> без сглаживания</span><span><b style="color: #2dd4bf">—</b> с {{chosen}}</span><span><b style="color: #fbbf24">┊</b> резонанс {{peak}} Гц</span></div>
      </div>
      <div style="display: flex; flex-direction: column; gap: 6px; min-height: 0">
        <div style="display: grid; grid-template-columns: 1.7fr 0.9fr 0.8fr 1.1fr; padding: 0 10px; font-size: 12px; font-weight: 700; color: #66707b"><span>ТИП</span><span>ГЦ</span><span>ВИБР.</span><span>УСКОР.</span></div>
""" + rows + """        <div style="flex: 1"></div>
        <a href="InputShaper.dc.html" style="height: 76px; border-radius: 20px; background: #2dd4bf; color: #042f2a; font-size: 17px; font-weight: 800; display: flex; flex-direction: column; align-items: center; justify-content: center"><span>Сохранить {{chosen}}</span><span style="font-size: 12px; font-weight: 600; opacity: 0.75">Klipper перезапустится</span></a>
      </div>
    </div>
  </main>
</div>
"""
    data = {}
    for ax, pk in (('x', 54.2), ('y', 41.8)):
        data[ax] = {'peak': str(pk), 'peakF': pk, 'raw': graphs[ax][0], 'curves': graphs[ax][1], 'rows': table[ax]}
    import json as _json
    js = """var DATA = """ + _json.dumps(data, ensure_ascii=False) + """;
class Component extends DCLogic {
  constructor(props) { super(props); this.state = { ax: 'x', pick: { x: 0, y: 1 } }; }
  renderVals() {
    var self = this, st = this.state, d = DATA[st.ax], sel = st.pick[st.ax];
    var rec = d.rows.map(function (r) { return r[4]; }).indexOf(true);
    var chosen = d.rows[sel];
    return {
      raw: d.raw, shapedPts: d.curves[sel], peak: d.peak, peakX: (10 + (d.peakF - 5) / 145 * 420).toFixed(1), chosen: chosen[0],
      graphAria: 'Ось ' + st.ax.toUpperCase() + ': резонанс ' + d.peak + ' герц, выбран ' + chosen[0],
      axes: ['x', 'y'].map(function (a) { var on = a === st.ax; return { label: a.toUpperCase(), sel: on ? 'true' : 'false', bg: on ? '#2dd4bf' : 'transparent', fg: on ? '#042f2a' : '#c7cdd4', pick: function () { self.setState({ ax: a }); } }; }),
      shapers: d.rows.map(function (r, i) {
        var on = i === sel;
        return { name: r[0] + (i === rec ? ' ★' : ''), freq: r[1], vib: r[2], acc: r[3], checked: on ? 'true' : 'false',
          bg: on ? 'rgba(45, 212, 191, 0.13)' : '#15181d', ring: on ? 'rgba(45, 212, 191, 0.5)' : 'transparent', fg: on ? '#5eead4' : '#c7cdd4',
          pick: function () { var p = Object.assign({}, st.pick); p[st.ax] = i; self.setState({ pick: p }); } };
      })
    };
  }
}"""
    write('ShaperResult', 'Вибрации · замер', body, js)
    gen.note_src('Вибрации · замер', 'graph', 'Rivet reads /tmp/calibration_data_<axis>_*.csv (written by SHAPER_CALIBRATE on the same host); table from the SHAPER_CALIBRATE response')

    # ---- Belts: two diagonal responses overlaid
    pa = [(78, 1.0, 6), (122, 0.3, 7)]
    pb = [(71, 0.86, 6), (118, 0.32, 7)]
    la = _poly(lambda f: _psd(f, pa), X0, Y0, W, H)
    lb = _poly(lambda f: _psd(f, pb), X0, Y0, W, H)
    svg_b = ('<svg role="img" aria-label="Ремень A: пик 78 герц, ремень B: пик 71 герц" viewBox="0 0 440 260" style="width: 100%%; height: 100%%">%s'
             '<polyline points="%s" fill="none" stroke="#2dd4bf" stroke-width="3" stroke-linejoin="round"></polyline>'
             '<polyline points="%s" fill="none" stroke="#f97316" stroke-width="3" stroke-linejoin="round"></polyline></svg>') % (_axes(X0, Y0, W, H), la, lb)
    body = root_open('tools') + """  <main style="flex: 1; min-width: 0; box-sizing: border-box; padding: 16px; display: flex; flex-direction: column; gap: 10px">
""" + header('Ремни', 'Calibrate.dc.html') + """    <div style="flex: 1; min-height: 0; display: grid; grid-template-columns: 1fr 250px; gap: 12px">
      <div style="border-radius: 20px; background: #15181d; padding: 8px; display: flex; flex-direction: column; min-height: 0">
        <div style="flex: 1; min-height: 0">""" + svg_b + """</div>
        <div style="display: flex; gap: 16px; padding: 4px 8px 2px; font-size: 13px; color: #98a1ac"><span><b style="color: #2dd4bf">—</b> ремень A · пик 78 Гц</span><span><b style="color: #f97316">—</b> ремень B · пик 71 Гц</span></div>
      </div>
      <div style="display: flex; flex-direction: column; gap: 10px">
        <div style="border-radius: 20px; background: #15181d; padding: 14px 16px; display: flex; flex-direction: column; gap: 2px"><span style="font-size: 13px; color: #98a1ac">Похожесть</span><span style="font-size: 36px; font-weight: 800; color: #fcd34d">87%</span><span style="font-size: 13px; color: #98a1ac">хорошо — от 90%</span></div>
        <div style="border-radius: 20px; background: rgba(251, 191, 36, 0.08); padding: 14px 16px; font-size: 15px; line-height: 1.45; color: #e2e6ea">Ремень <b>B</b> слабее: его пик ниже. Подтяните натяжитель B на четверть оборота и проверьте снова.</div>
        <div style="flex: 1"></div>
        <a href="Belts.dc.html" style="height: 76px; border-radius: 20px; background: #2dd4bf; color: #042f2a; font-size: 17px; font-weight: 800; display: flex; flex-direction: column; align-items: center; justify-content: center"><span>Проверить снова</span><span style="font-size: 12px; font-weight: 600; opacity: 0.75">~3 мин · не трогайте принтер</span></a>
      </div>
    </div>
  </main>
</div>
"""
    write('Belts', 'Ремни', body, EMPTY)
    gen.note_src('Ремни', 'curves', 'TEST_RESONANCES AXIS=1,1 and AXIS=1,-1 OUTPUT=raw_data (core Klipper); Rivet computes PSD of /tmp/raw_data_*.csv and compares — no custom macros')


ROW_LINK = """        <a href="{href}" style="min-height: 72px; flex: none; border-radius: 20px; background: #15181d; color: #f4f6f8; display: flex; align-items: center; gap: 14px; padding: 0 18px">
          <span style="flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px"><span style="font-size: 17px; font-weight: 700">{t}</span><span style="font-size: 14px; color: #98a1ac">{d}</span></span>
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="#66707b" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m9 6 6 6-6 6"></path></svg>
        </a>
"""


def build_leftovers():
    # ---- Service: progress bars instead of "82 / 300 ч"
    items = [('Фильтр HEPA', 82, 300, 'заменить через 218 ч', '#2dd4bf'),
             ('Сопло', 311, 500, 'заменить через 189 ч', '#2dd4bf'),
             ('Направляющие', 96, 100, 'пора смазать', '#fbbf24')]
    rows = ''
    for name, used, every, note, col in items:
        pct = min(100, used * 100 // every)
        rows += """        <div style="flex: none; border-radius: 20px; background: #15181d; padding: 14px 18px; display: flex; flex-direction: column; gap: 10px">
          <div style="display: flex; align-items: baseline; gap: 10px"><span style="flex: 1; font-size: 17px; font-weight: 700">%s</span><span style="font-size: 14px; color: %s; font-weight: 600">%s</span></div>
          <div role="progressbar" aria-label="%s" aria-valuenow="%d" aria-valuemin="0" aria-valuemax="%d" style="height: 10px; border-radius: 5px; background: #252a31; overflow: hidden"><span style="display: block; height: 10px; width: %d%%; border-radius: 5px; background: %s"></span></div>
          <div style="display: flex; align-items: center; gap: 10px"><span style="flex: 1; font-size: 13px; color: #98a1ac; font-variant-numeric: tabular-nums">%d из %d ч печати</span><button type="button" style="height: 44px; padding: 0 14px; border: 0; border-radius: 14px; background: #1f242b; color: #2dd4bf; font: 700 14px Inter, system-ui, sans-serif; cursor: pointer">Заменил</button></div>
        </div>
""" % (name, '#fcd34d' if col == '#fbbf24' else '#98a1ac', note, name, used, every, pct, col, used, every)
    body = root_open('tools') + """  <main style="flex: 1; min-width: 0; box-sizing: border-box; padding: 16px 16px 0; display: flex; flex-direction: column; gap: 10px">
""" + header('Сервис', 'Tools.dc.html') + """    <div class="scroll" style="flex: 1; min-height: 0; display: flex; flex-direction: column; gap: 8px; padding-bottom: 16px">
""" + rows + """        <span style="margin: 4px 4px 0; font-size: 14px; color: #98a1ac">Часы считаются по истории печати. Интервал меняется тапом по строке.</span>
    </div>
  </main>
</div>
"""
    write('Maintenance', 'Сервис', body, EMPTY)
    gen.note_src('Сервис', 'hours', 'Rivet: sum of print_duration from server.history since the last «Заменил»; stored in Moonraker database')

    # ---- Camera: real empty state, timelapse only when a camera exists
    body = root_open('tools') + """  <main style="flex: 1; min-width: 0; box-sizing: border-box; padding: 16px; display: flex; flex-direction: column; gap: 12px">
""" + header('Камера', 'Tools.dc.html') + """    <div style="flex: 1; border-radius: 24px; background: #15181d; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 12px; text-align: center; padding: 24px">
      <span aria-hidden="true" style="width: 72px; height: 72px; border-radius: 24px; background: #1f242b; color: #98a1ac; display: flex; align-items: center; justify-content: center">""" + svg('camera', 36) + """</span>
      <span style="font-size: 24px; font-weight: 800">Камера не подключена</span>
      <span style="max-width: 440px; font-size: 16px; line-height: 1.5; color: #98a1ac">Добавьте камеру в Moonraker — здесь появится картинка с принтера, а в печати — таймлапс.</span>
    </div>
  </main>
</div>
"""
    write('Camera', 'Камера', body, EMPTY)
    gen.note_src('Камера', 'stream', 'server.webcams.list; timelapse settings only when a webcam and moonraker-timelapse exist')

    # ---- Bed screws: top view with a badge per screw
    def screw(pos, title, val, note, kind):
        col = {'ref': ('#1f242b', '#98a1ac'), 'cw': ('#123a35', '#5eead4'), 'ok': ('#182320', '#98a1ac')}[kind]
        return """        <div style="position: absolute; %s; width: 150px; border-radius: 16px; background: %s; padding: 10px 12px; display: flex; flex-direction: column; gap: 2px">
          <span style="font-size: 13px; color: #98a1ac">%s</span><span style="font-size: 20px; font-weight: 800; color: %s; font-variant-numeric: tabular-nums">%s</span><span style="font-size: 12px; color: #98a1ac">%s</span>
        </div>
""" % (pos, col[0], title, col[1], val, note)
    body = root_open('tools') + """  <main style="flex: 1; min-width: 0; box-sizing: border-box; padding: 16px; display: flex; flex-direction: column; gap: 12px">
""" + header('Винты стола', 'Calibrate.dc.html') + """    <div style="flex: 1; min-height: 0; display: grid; grid-template-columns: 1fr 230px; gap: 14px">
      <div role="img" aria-label="Стол сверху: задний левый опорный, задний правый четверть оборота по часовой, передний левый треть оборота против, передний правый ровно" style="position: relative; border-radius: 24px; background: #15181d">
        <div aria-hidden="true" style="position: absolute; inset: 40px 70px; border-radius: 14px; border: 2px dashed #2a3038"></div>
        <span aria-hidden="true" style="position: absolute; left: 50%; top: 50%; transform: translate(-50%, -50%); font-size: 13px; color: #66707b">стол, вид сверху</span>
""" + screw('left: 14px; top: 14px', 'Задний левый', 'опорный', 'не крутить', 'ref') + screw('right: 14px; top: 14px', 'Задний правый', '↻ ¼', 'по часовой', 'cw') + screw('left: 14px; bottom: 14px', 'Передний левый', '↺ ⅓', 'против часовой', 'cw') + screw('right: 14px; bottom: 14px', 'Передний правый', '✓', 'ровно', 'ok') + """      </div>
      <div style="display: flex; flex-direction: column; gap: 10px">
        <span style="font-size: 15px; line-height: 1.5; color: #c7cdd4">Подкрутите винты на столько, сколько показано, и проверьте снова. Повторять, пока все не станут «ровно».</span>
        <div style="flex: 1"></div>
        <a href="Screws.dc.html" style="height: 80px; border-radius: 22px; background: #2dd4bf; color: #042f2a; font-size: 18px; font-weight: 800; display: flex; flex-direction: column; align-items: center; justify-content: center"><span>Проверить снова</span><span style="font-size: 12px; font-weight: 600; opacity: 0.75">датчик коснётся у каждого винта</span></a>
      </div>
    </div>
  </main>
</div>
"""
    write('Screws', 'Винты стола', body, EMPTY)
    gen.note_src('Винты стола', 'turns', 'SCREWS_TILT_CALCULATE response (00:15 = quarter turn); only when [screws_tilt_adjust] exists')

    # ---- Macros: run simple macros straight from the list, with confirm
    body = root_open('tools') + """  <main style="flex: 1; min-width: 0; box-sizing: border-box; padding: 16px 16px 0; display: flex; flex-direction: column; gap: 10px">
""" + header('Макросы', 'Tools.dc.html') + """    <div class="scroll" style="flex: 1; min-height: 0; display: flex; flex-direction: column; gap: 8px; padding-bottom: 16px">
      <sc-for list="{{groups}}" as="g" hint-placeholder-count="2">
        <span style="margin: 6px 4px 0; font-size: 13px; font-weight: 700; letter-spacing: 0.04em; color: #66707b">{{g.title}}</span>
        <sc-for list="{{g.items}}" as="m" hint-placeholder-count="3">
          <div style="min-height: 72px; flex: none; border-radius: 20px; background: #15181d; display: flex; align-items: center; gap: 10px; padding: 0 8px 0 18px">
            <span style="flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px"><span style="font-size: 16px; font-weight: 700; font-family: ui-monospace, Menlo, monospace">{{m.name}}</span><span style="font-size: 14px; color: #98a1ac">{{m.desc}}</span></span>
            <sc-if value="{{m.hasParams}}" hint-placeholder-val="{{ false }}"><a href="MacroRun.dc.html" aria-label="{{m.openAria}}" style="height: 56px; padding: 0 16px; border-radius: 16px; background: #1f242b; color: #f4f6f8; font-size: 15px; font-weight: 700; display: flex; align-items: center">Параметры ›</a></sc-if>
            <sc-if value="{{m.simple}}" hint-placeholder-val="{{ true }}"><button type="button" onClick="{{m.run}}" aria-label="{{m.runAria}}" style="height: 56px; padding: 0 18px; border: 0; border-radius: 16px; background: rgba(45, 212, 191, 0.13); color: #2dd4bf; font: 800 15px Inter, system-ui, sans-serif; cursor: pointer">Запустить</button></sc-if>
          </div>
        </sc-for>
      </sc-for>
    </div>
  </main>
  <sc-if value="{{confirmOpen}}" hint-placeholder-val="{{ false }}">
    <div role="alertdialog" aria-modal="true" aria-labelledby="mt" style="position: absolute; inset: 0; background: rgba(5, 6, 8, 0.78); display: flex; align-items: center; justify-content: center">
      <div style="width: 520px; border-radius: 28px; background: #15181d; padding: 24px; display: flex; flex-direction: column; gap: 12px">
        <h2 id="mt" style="margin: 0; font-size: 24px; font-weight: 800">Запустить {{pending}}?</h2>
        <p style="margin: 0; font-size: 15px; color: #98a1ac">Подтверждение можно выключить: Настройки › Безопасность.</p>
        <div style="display: grid; grid-template-columns: 1fr 1.3fr; gap: 10px; margin-top: 6px">
          <button type="button" onClick="{{cancel}}" style="height: 76px; border: 0; border-radius: 20px; background: #1f242b; color: #f4f6f8; font: 700 17px Inter, system-ui, sans-serif; cursor: pointer">Отмена</button>
          <button type="button" onClick="{{confirm}}" style="height: 76px; border: 0; border-radius: 20px; background: #2dd4bf; color: #042f2a; font: 800 17px Inter, system-ui, sans-serif; cursor: pointer">Запустить</button>
        </div>
      </div>
    </div>
  </sc-if>
</div>
"""
    js = """class Component extends DCLogic {
  constructor(props) { super(props); this.state = { pending: null }; }
  renderVals() {
    var self = this;
    var fav = [['LOAD_FILAMENT', 'Manual filament loading', false], ['AUTO_BED_LEVEL', 'BED_TEMP · EXTRUDER_TEMP · PROFILE', true]];
    var all = [['AIR_CIRCULATION_INTERNAL', 'Turn on internal air circulation', false], ['PURGE_FILAMENT', 'выдавить 25 мм', false], ['LCD_ON', 'BACKLIGHT', true], ['PLAY_MIDI', 'FILE', true]];
    function map(list) { return list.map(function (m) { return { name: m[0], desc: m[1], hasParams: m[2], simple: !m[2],
      openAria: m[0] + ': параметры', runAria: 'Запустить ' + m[0], run: function () { self.setState({ pending: m[0] }); } }; }); }
    return {
      groups: [{ title: 'ИЗБРАННЫЕ', items: map(fav) }, { title: 'ВСЕ', items: map(all) }],
      confirmOpen: !!this.state.pending, pending: this.state.pending || '',
      cancel: function () { self.setState({ pending: null }); },
      confirm: function () { self.setState({ pending: null }); }
    };
  }
}"""
    write('Macros', 'Макросы', body, js)
    gen.note_src('Макросы', 'list', 'printer.objects.list → gcode_macro * (hidden: names starting with _); params parsed from the macro gcode (params.X)')
