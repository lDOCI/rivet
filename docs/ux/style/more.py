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
""" + header('Z-offset бумагой', 'ProbeZ.dc.html') + """    <div style="flex: 1; min-height: 0; display: grid; grid-template-columns: 1fr 260px; gap: 16px">
      <div style="display: flex; flex-direction: column; gap: 12px; min-width: 0">
        <p style="margin: 0; font-size: 17px; line-height: 1.5; color: #c7cdd4">Лист бумаги под сопло. Опускайте, пока бумага не начнёт цепляться, но ещё двигается.</p>
        <div style="border-radius: 22px; background: #15181d; padding: 14px 18px; display: flex; align-items: center; gap: 10px"><span style="font-size: 15px; color: #98a1ac">Сопло над столом, мм</span><span style="flex: 1"></span><span style="font-size: 40px; font-weight: 800; font-variant-numeric: tabular-nums">{{zLabel}}</span></div>
        <span style="font-size: 14px; font-weight: 600; color: #98a1ac">Шаг, мм</span>
        <div role="radiogroup" aria-label="Шаг" style="display: flex; gap: 6px">
          <sc-for list="{{steps}}" as="o" hint-placeholder-count="5"><button type="button" role="radio" aria-checked="{{o.checked}}" onClick="{{o.pick}}" style="flex: 1; height: 64px; border: 0; border-radius: 16px; background: {{o.bg}}; color: {{o.fg}}; font: 700 16px Inter, system-ui, sans-serif; cursor: pointer">{{o.label}}</button></sc-for>
        </div>
        <div style="flex: 1"></div>
        <div style="display: grid; grid-template-columns: 1fr 1.4fr; gap: 10px">
          <a href="ProbeZ.dc.html" style="height: 76px; border-radius: 20px; background: rgba(251, 75, 94, 0.14); color: #fb7185; font-size: 17px; font-weight: 700; display: flex; align-items: center; justify-content: center">Отменить</a>
          <a href="ProbeZ.dc.html" style="height: 76px; border-radius: 20px; background: #2dd4bf; color: #042f2a; font-size: 17px; font-weight: 800; display: flex; flex-direction: column; align-items: center; justify-content: center"><span>Принять</span><span style="font-size: 12px; font-weight: 600; opacity: 0.75">Klipper перезапустится</span></a>
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
  constructor(props) { super(props); this.state = { z: 0.45, step: 0.05 }; }
  renderVals() {
    var self = this, st = this.state;
    var opts = [0.005, 0.01, 0.05, 0.1, 1];
    return {
      zLabel: st.z.toFixed(3).replace('.', ','),
      upZ: function () { self.setState({ z: +(st.z + st.step).toFixed(3) }); },
      downZ: function () { self.setState({ z: Math.max(0, +(st.z - st.step).toFixed(3)) }); },
      steps: opts.map(function (v) { var on = v === st.step; return { label: String(v).replace('.', ','), checked: on ? 'true' : 'false', bg: on ? '#2dd4bf' : '#15181d', fg: on ? '#042f2a' : '#f4f6f8', pick: function () { self.setState({ step: v }); } }; })
    };
  }
}"""
    write('ZWizard', 'Z-offset бумагой', body, js)
    gen.note_src('Z-offset бумагой', 'height', 'PROBE_CALIBRATE, then TESTZ Z=±step, ACCEPT or ABORT; toolhead.position[2]')

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
