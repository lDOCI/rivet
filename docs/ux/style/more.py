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
    for ax, peaks, fs in (('x', px, 54.2), ('y', py, 41.8)):
        graphs[ax] = (_poly(lambda f: _psd(f, peaks), X0, Y0, W, H), _poly(shaped(peaks, fs), X0, Y0, W, H))
    svg_tpl = ('<svg role="img" aria-label="{{graphAria}}" viewBox="0 0 440 260" style="width: 100%%; height: 100%%">%s'
               '<polyline points="{{raw}}" fill="none" stroke="#98a1ac" stroke-width="2" stroke-dasharray="4 4" stroke-linejoin="round"></polyline>'
               '<polyline points="{{shapedPts}}" fill="none" stroke="#2dd4bf" stroke-width="3" stroke-linejoin="round"></polyline>'
               '<path d="M{{peakX}} 10 V240" stroke="#fbbf24" stroke-width="1.5" stroke-dasharray="3 3"></path></svg>') % _axes(X0, Y0, W, H)
    rows = """          <sc-for list="{{shapers}}" as="t" hint-placeholder-count="5">
            <div style="height: 40px; flex: none; border-radius: 12px; background: {{t.bg}}; padding: 0 10px; display: grid; grid-template-columns: 1.7fr 0.9fr 0.8fr 1.1fr; align-items: center; font-size: 13px; font-variant-numeric: tabular-nums; color: {{t.fg}}">
              <span style="font-weight: 800">{{t.name}}</span><span>{{t.freq}}</span><span>{{t.vib}}</span><span>{{t.acc}}</span>
            </div>
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
    data = {
        'x': {'peak': '54.2', 'peakF': 54.2, 'raw': graphs['x'][0], 'shaped': graphs['x'][1],
              'rows': [['MZV', '54.2', '1%', '7 400', True], ['EI', '62.0', '0%', '6 100', False], ['ZV', '58.6', '4%', '11 200', False], ['2HUMP_EI', '74.4', '0%', '4 900', False], ['3HUMP_EI', '89.2', '0%', '4 300', False]]},
        'y': {'peak': '41.8', 'peakF': 41.8, 'raw': graphs['y'][0], 'shaped': graphs['y'][1],
              'rows': [['MZV', '40.4', '3%', '4 100', False], ['EI', '41.8', '1%', '5 900', True], ['ZV', '44.0', '7%', '6 700', False], ['2HUMP_EI', '55.6', '0%', '2 800', False], ['3HUMP_EI', '66.8', '0%', '2 400', False]]},
    }
    import json as _json
    js = """var DATA = """ + _json.dumps(data, ensure_ascii=False) + """;
class Component extends DCLogic {
  constructor(props) { super(props); this.state = { ax: 'x' }; }
  renderVals() {
    var self = this, d = DATA[this.state.ax];
    var chosen = d.rows.filter(function (r) { return r[4]; })[0];
    return {
      raw: d.raw, shapedPts: d.shaped, peak: d.peak, peakX: (10 + (d.peakF - 5) / 145 * 420).toFixed(1), chosen: chosen[0],
      graphAria: 'Ось ' + this.state.ax.toUpperCase() + ': резонанс ' + d.peak + ' герц, рекомендовано ' + chosen[0],
      axes: ['x', 'y'].map(function (a) { var on = a === self.state.ax; return { label: a.toUpperCase(), sel: on ? 'true' : 'false', bg: on ? '#2dd4bf' : 'transparent', fg: on ? '#042f2a' : '#c7cdd4', pick: function () { self.setState({ ax: a }); } }; }),
      shapers: d.rows.map(function (r) { return { name: r[0], freq: r[1], vib: r[2], acc: r[3], bg: r[4] ? 'rgba(45, 212, 191, 0.13)' : '#15181d', fg: r[4] ? '#5eead4' : '#c7cdd4' }; })
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
