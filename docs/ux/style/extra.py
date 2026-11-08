import gen
from gen import write, root_open, header, svg

KB_HTML = """      <div role="group" aria-label="Клавиатура" style="flex: none; display: flex; flex-direction: column; gap: 6px">
        <sc-for list="{{kbRows}}" as="row" hint-placeholder-count="5">
          <div style="display: flex; gap: 6px">
            <sc-for list="{{row.keys}}" as="k" hint-placeholder-count="10">
              <button type="button" onClick="{{k.press}}" aria-label="{{k.aria}}" style="flex: {{k.flex}}; height: 50px; border: 0; border-radius: 12px; background: {{k.bg}}; color: {{k.fg}}; font: 600 19px Inter, system-ui, sans-serif; cursor: pointer">{{k.label}}</button>
            </sc-for>
          </div>
        </sc-for>
      </div>
"""

KB_JS = r"""
  kbRows(onDone) {
    var self = this, st = this.state;
    var letters = [['1','2','3','4','5','6','7','8','9','0'], ['q','w','e','r','t','y','u','i','o','p'], ['a','s','d','f','g','h','j','k','l','⌫'], ['⇧','z','x','c','v','b','n','m','=','.'], ['#+=','_','-',' ','⏎']];
    var syms = [['1','2','3','4','5','6','7','8','9','0'], ['!','@','#','$','%','&','*','(',')','/'], ['"',"'",';',',','?','+','[',']','<','⌫'], ['⇧','>','{','}','|','~','^','`','=','.'], ['abc','_',':',' ','⏎']];
    var set = st.kbSym ? syms : letters;
    return set.map(function (row) {
      return { keys: row.map(function (k) {
        var special = k === '⌫' || k === '⇧' || k === '⏎' || k === 'abc' || k === '#+=';
        var label = k === ' ' ? 'пробел' : (k.length === 1 && k !== '_' && st.kbUpper && !st.kbSym ? k.toUpperCase() : k);
        return {
          label: label, aria: k === '⌫' ? 'Стереть' : k === '⇧' ? 'Регистр' : k === '⏎' ? 'Готово' : k === ' ' ? 'Пробел' : label,
          flex: k === ' ' ? 4 : (k === '⏎' ? 2 : 1),
          bg: k === '⏎' ? '#2dd4bf' : (k === '⇧' && st.kbUpper) ? '#3a414a' : special ? '#1f242b' : '#252a31',
          fg: k === '⏎' ? '#042f2a' : '#f4f6f8',
          press: function () {
            var t = self.state.kbText;
            if (k === '⌫') self.setState({ kbText: t.slice(0, -1) });
            else if (k === '⇧') self.setState({ kbUpper: !self.state.kbUpper });
            else if (k === '⏎') onDone(t);
            else if (k === '#+=') self.setState({ kbSym: true });
            else if (k === 'abc') self.setState({ kbSym: false });
            else self.setState({ kbText: t + (self.state.kbUpper && !self.state.kbSym ? k.toUpperCase() : k) });
          }
        };
      }) };
    });
  }
"""

EMPTY = 'class Component extends DCLogic {\n  renderVals() {\n    return {};\n  }\n}'

def build_extra():
    build_limits()
    # E-stop confirm
    body = root_open('none') + '''  <main role="alertdialog" aria-labelledby="t" style="flex: 1; box-sizing: border-box; padding: 28px 40px; display: flex; flex-direction: column; justify-content: center; gap: 14px">
    <span aria-hidden="true" style="width: 72px; height: 72px; border-radius: 24px; background: rgba(251, 75, 94, 0.16); color: #fb7185; display: flex; align-items: center; justify-content: center">''' + svg('warn', 36) + '''</span>
    <h1 id="t" style="margin: 0; font-size: 32px; font-weight: 800; letter-spacing: -0.02em">Аварийная остановка?</h1>
    <p style="margin: 0; max-width: 560px; font-size: 18px; line-height: 1.5; color: #c7cdd4">Klipper сразу выключит нагрев и моторы. Печать продолжить не получится, потом нужен перезапуск прошивки.</p>
    <div style="margin-top: 12px; display: grid; grid-template-columns: 1fr 1.3fr; gap: 12px">
      <a href="Main.dc.html" style="height: 84px; border-radius: 22px; background: #15181d; color: #f4f6f8; font-size: 18px; font-weight: 700; display: flex; align-items: center; justify-content: center">Отмена</a>
      <a href="KlipperError.dc.html" style="height: 84px; border-radius: 22px; background: #e11d48; color: #ffffff; font-size: 19px; font-weight: 800; display: flex; align-items: center; justify-content: center">Остановить всё</a>
    </div>
    <span style="font-size: 14px; color: #66707b">Подтверждение можно отключить: Настройки › Безопасность.</span>
  </main>
</div>
'''
    write('EStop', 'Аварийная остановка', body, EMPTY)

    # Console with on-screen keyboard
    chips = ''.join('<button type="button" onClick="{{q%d}}" style="height: 56px; padding: 0 16px; border: 0; border-radius: 16px; background: #15181d; color: #f4f6f8; font: 600 15px ui-monospace, Menlo, monospace; cursor: pointer">%s</button>' % (i, c)
                    for i, c in enumerate(['G28', 'M105', 'QUERY_ENDSTOPS', 'STATUS', 'SAVE_CONFIG']))
    body = root_open('tools') + """  <main style="flex: 1; min-width: 0; box-sizing: border-box; padding: 16px; display: flex; flex-direction: column; gap: 8px">
<sc-if value="{{kbClosed}}" hint-placeholder-val="{{ true }}">
""" + header('Консоль', 'Tools.dc.html', '<button type="button" onClick="{{clear}}" style="height: 64px; padding: 0 18px; border: 0; border-radius: 20px; background: #15181d; color: #c7cdd4; font: 600 15px Inter, system-ui, sans-serif; cursor: pointer">Очистить</button>') + """</sc-if>
    <div role="log" aria-label="Ответы Klipper" class="scroll" style="flex: 1; min-height: 0; border-radius: 20px; background: #070809; padding: 12px 16px; display: flex; flex-direction: column; gap: 6px; font: 15px/1.4 ui-monospace, SFMono-Regular, Menlo, monospace">
      <sc-for list="{{log}}" as="l" hint-placeholder-count="6"><span style="color: {{l.c}}">{{l.t}}</span></sc-for>
    </div>
    <sc-if value="{{kbClosed}}" hint-placeholder-val="{{ true }}"><div style="display: flex; gap: 8px; overflow-x: auto">""" + chips + """</div></sc-if>
    <div style="display: flex; gap: 8px">
      <button type="button" onClick="{{openKb}}" aria-label="Команда, нажмите чтобы ввести" style="flex: 1; height: 60px; border: 0; border-radius: 18px; background: #15181d; box-shadow: inset 0 0 0 2px {{fieldRing}}; color: {{fieldInk}}; display: flex; align-items: center; padding: 0 18px; font: 17px ui-monospace, Menlo, monospace; text-align: left; cursor: text">{{fieldText}}</button>
      <button type="button" onClick="{{send}}" style="width: 120px; border: 0; border-radius: 18px; background: #2dd4bf; color: #042f2a; font: 800 17px Inter, system-ui, sans-serif; cursor: pointer">Отправить</button>
    </div>
<sc-if value="{{kbOpen}}" hint-placeholder-val="{{ false }}">
""" + KB_HTML + """</sc-if>
  </main>
</div>
"""
    js = """class Component extends DCLogic {
  constructor(props) {
    super(props);
    this.state = { kbOpen: false, kbText: '', kbUpper: true, kbSym: false, log: [['> G28', '#5eead4'], ['ok', '#98a1ac'], ['> QUERY_ENDSTOPS', '#5eead4'], ['x:open y:open z:TRIGGERED', '#c7cdd4'], ['> M105', '#5eead4'], ['ok T:24.1 /0.0 B:23.4 /0.0', '#c7cdd4'], ['// Notice: z-offset should not be saved to endstop', '#fcd34d'], ['!! Move out of range: 0.000 0.000 -12.000', '#fb7185']] };
  }
""" + KB_JS + """
  renderVals() {
    var self = this, st = this.state;
    function run(cmd) { if (!cmd) return; self.setState({ log: self.state.log.concat([['> ' + cmd, '#5eead4'], ['ok', '#98a1ac']]), kbText: '', kbOpen: false }); }
    var v = {
      kbOpen: st.kbOpen, kbClosed: !st.kbOpen,
      fieldText: st.kbText ? st.kbText + (st.kbOpen ? '▏' : '') : (st.kbOpen ? '▏' : 'G-code или макрос'),
      fieldInk: st.kbText ? '#f4f6f8' : '#66707b', fieldRing: st.kbOpen ? '#2dd4bf' : 'transparent',
      log: st.log.map(function (l) { return { t: l[0], c: l[1] }; }),
      kbRows: this.kbRows(run),
      openKb: function () { self.setState({ kbOpen: true }); },
      send: function () { run(self.state.kbText); },
      clear: function () { self.setState({ log: [] }); }
    };
    ['G28', 'M105', 'QUERY_ENDSTOPS', 'STATUS', 'SAVE_CONFIG'].forEach(function (c, i) { v['q' + i] = function () { run(c); }; });
    return v;
  }
}"""
    write('Console', 'Консоль', body, js)
    gen.note_src('Консоль', 'log', 'notify_gcode_response + server.gcode_store; send: printer.gcode.script')

    # Lock screen with PIN
    keys = ''.join('<button type="button" style="border: 0; border-radius: 18px; background: #1b1f25; color: #f4f6f8; font: 700 26px Inter, system-ui, sans-serif; cursor: pointer">%s</button>' % k for k in ['1','2','3','4','5','6','7','8','9','','0','⌫'])
    keys = keys.replace('<button type="button" style="border: 0; border-radius: 18px; background: #1b1f25; color: #f4f6f8; font: 700 26px Inter, system-ui, sans-serif; cursor: pointer"></button>', '<span></span>')
    body = '''<div style="width: 800px; height: 480px; box-sizing: border-box; padding: 28px 36px; display: grid; grid-template-columns: 1fr 300px; gap: 36px; background: #0b0d10; color: #f4f6f8; font-family: Inter, system-ui, sans-serif">
  <div style="display: flex; flex-direction: column; justify-content: center; gap: 10px">
    <span aria-hidden="true" style="color: #98a1ac">''' + svg('lock', 34) + '''</span>
    <span style="font-size: 30px; font-weight: 800">Экран заблокирован</span>
    <span style="font-size: 17px; color: #98a1ac">Печать идёт: 42% · осталось 48 мин</span>
    <div style="margin-top: 14px; display: flex; gap: 12px" aria-label="Введено 2 цифры из 4"><span style="width: 18px; height: 18px; border-radius: 9px; background: #2dd4bf"></span><span style="width: 18px; height: 18px; border-radius: 9px; background: #2dd4bf"></span><span style="width: 18px; height: 18px; border-radius: 9px; background: #2a3038"></span><span style="width: 18px; height: 18px; border-radius: 9px; background: #2a3038"></span></div>
    <div style="flex: 1"></div>
    <a href="EStop.dc.html" style="align-self: flex-start; height: 64px; padding: 0 22px; border-radius: 20px; background: rgba(251, 75, 94, 0.16); color: #fb7185; font-size: 16px; font-weight: 800; display: flex; align-items: center">СТОП работает и здесь</a>
  </div>
  <div style="display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); grid-template-rows: repeat(4, minmax(0, 1fr)); gap: 8px">''' + keys + '''</div>
</div>
'''
    write('Lock', 'Блокировка', body, EMPTY)

    # Prompt from macro (action:prompt_*) over a screen
    body = root_open('home') + '''  <main style="flex: 1"></main>
  <div role="dialog" aria-modal="true" aria-labelledby="pt" style="position: absolute; inset: 0; background: rgba(5, 6, 8, 0.78); display: flex; align-items: center; justify-content: center">
    <div style="width: 560px; border-radius: 28px; background: #15181d; padding: 24px; display: flex; flex-direction: column; gap: 12px">
      <span style="font-size: 13px; font-weight: 700; letter-spacing: 0.04em; color: #66707b">ИЗ МАКРОСА</span>
      <h2 id="pt" style="margin: 0; font-size: 26px; font-weight: 800">Смена филамента</h2>
      <p style="margin: 0; font-size: 17px; line-height: 1.5; color: #c7cdd4">Вставьте новый пруток и нажмите «Загрузить». Когда пойдёт чистый цвет — «Продолжить».</p>
      <div style="display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 10px; margin-top: 6px">
        <button type="button" style="height: 76px; border: 0; border-radius: 20px; background: #1f242b; color: #f4f6f8; font: 700 17px Inter, system-ui, sans-serif; cursor: pointer">Выгрузить</button>
        <button type="button" style="height: 76px; border: 0; border-radius: 20px; background: #1f242b; color: #f4f6f8; font: 700 17px Inter, system-ui, sans-serif; cursor: pointer">Загрузить</button>
        <button type="button" style="height: 76px; border: 0; border-radius: 20px; background: #2dd4bf; color: #042f2a; font: 800 17px Inter, system-ui, sans-serif; cursor: pointer">Продолжить</button>
      </div>
    </div>
  </div>
</div>
'''
    write('Prompt', 'Диалог макроса', body, EMPTY)
    gen.note_src('Диалог макроса', 'title/text/buttons', 'action:prompt_begin/_text/_button/_show via notify_gcode_response')

    # Temperature graph
    import math
    def poly(f):
        return ' '.join('%d,%d' % (20 + i * 6, 260 - f(i)) for i in range(100))
    nozzle = poly(lambda i: min(220, 24 + i * 4.2) if i < 52 else 220 + 2 * math.sin(i / 2.0))
    bed = poly(lambda i: min(80, 23 + i * 1.4) if i < 42 else 80 + math.sin(i / 3.0))
    body = root_open('control') + '''  <main style="flex: 1; min-width: 0; box-sizing: border-box; padding: 16px; display: flex; flex-direction: column; gap: 10px">
''' + header('График температур', 'Heating.dc.html', '<span style="font-size: 15px; color: #98a1ac">последние 10 мин</span>') + '''    <svg role="img" aria-label="Сопло дошло до 220 градусов, стол до 80" viewBox="0 0 640 280" style="flex: 1; min-height: 0; width: 100%; border-radius: 20px; background: #15181d">
      <path d="M20 60H620M20 120H620M20 180H620M20 240H620" stroke="#1f242b" stroke-width="1"></path>
      <text x="26" y="54" fill="#66707b" font-size="12" font-family="Inter, sans-serif">250°</text><text x="26" y="174" fill="#66707b" font-size="12" font-family="Inter, sans-serif">100°</text>
      <polyline points="''' + nozzle + '''" fill="none" stroke="#f97316" stroke-width="3" stroke-linejoin="round"></polyline>
      <polyline points="''' + bed + '''" fill="none" stroke="#60a5fa" stroke-width="3" stroke-linejoin="round"></polyline>
    </svg>
    <div style="display: flex; gap: 20px; font-size: 15px; color: #c7cdd4"><span><b style="color: #f97316">●</b> Сопло 220° / 220°</span><span><b style="color: #60a5fa">●</b> Стол 80° / 80°</span><span><b style="color: #98a1ac">●</b> MCU 41°</span></div>
  </main>
</div>
'''
    write('TempGraph', 'График температур', body, EMPTY)
    gen.note_src('График температур', 'series', 'server.temperature_store (+ live notify_status_update)')

    body = root_open('settings') + """  <main style="flex: 1; min-width: 0; box-sizing: border-box; padding: 16px; display: flex; flex-direction: column; gap: 10px">
""" + header('Сеть «HomeNet»', 'Network.dc.html') + """    <div style="display: flex; gap: 8px">
      <div aria-label="Пароль" style="flex: 1; height: 64px; border-radius: 18px; background: #15181d; box-shadow: inset 0 0 0 2px #2dd4bf; display: flex; align-items: center; padding: 0 18px; font: 600 20px Inter, system-ui, sans-serif; letter-spacing: 0.06em; color: {{fieldInk}}">{{fieldText}}</div>
      <button type="button" onClick="{{toggleShow}}" style="width: 120px; border: 0; border-radius: 18px; background: #15181d; color: #c7cdd4; font: 600 15px Inter, system-ui, sans-serif; cursor: pointer">{{showLabel}}</button>
      <a href="Network.dc.html" style="width: 150px; border-radius: 18px; background: {{okBg}}; color: {{okFg}}; font-size: 17px; font-weight: 800; display: flex; align-items: center; justify-content: center">Подключиться</a>
    </div>
    <div style="flex: 1"></div>
""" + KB_HTML + """  </main>
</div>
"""
    js = """class Component extends DCLogic {
  constructor(props) {
    super(props);
    this.state = { kbText: '', kbUpper: false, kbSym: false, show: false };
  }
""" + KB_JS + """
  renderVals() {
    var self = this, st = this.state;
    var t = st.kbText;
    return {
      fieldText: t ? (st.show ? t : t.replace(/./g, '•')) + '▏' : 'Пароль Wi-Fi',
      fieldInk: t ? '#f4f6f8' : '#66707b',
      showLabel: st.show ? 'Скрыть' : 'Показать',
      okBg: t.length >= 8 ? '#2dd4bf' : '#1f242b', okFg: t.length >= 8 ? '#042f2a' : '#66707b',
      toggleShow: function () { self.setState({ show: !st.show }); },
      kbRows: this.kbRows(function () {})
    };
  }
}"""
    write('WifiJoin', 'Подключение к Wi-Fi', body, js)


def build_limits():
    card = """        <sc-for list="{{%s}}" as="m" hint-placeholder-count="2">
          <div style="border-radius: 22px; background: #15181d; box-shadow: inset 0 0 0 2px {{m.ring}}; padding: 12px; display: flex; flex-direction: column; gap: 10px; %s">
            <span style="min-height: 32px; padding: 0 6px; display: flex; align-items: center; gap: 8px"><span style="flex: 1; display: flex; align-items: baseline; gap: 8px; min-width: 0"><span style="font-size: %s; font-weight: 700">{{m.title}}</span><span style="font-size: 14px; font-weight: 600; color: #66707b; white-space: nowrap">{{m.unit}}</span></span><sc-if value="{{m.changed}}" hint-placeholder-val="{{ false }}"><button type="button" onClick="{{m.reset}}" aria-label="{{m.resetAria}}" style="height: 32px; padding: 0 12px; border: 0; border-radius: 16px; background: rgba(45, 212, 191, 0.13); color: #5eead4; font: 600 13px Inter, system-ui, sans-serif; display: flex; align-items: center; gap: 6px; cursor: pointer"><svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 12a8 8 0 1 0 2.3-5.7"></path><path d="M4 4v4h4"></path></svg>{{m.was}}</button></sc-if></span>
            <span style="padding: 0 6px; margin-top: -6px; font-size: 14px; color: #98a1ac">{{m.hint}}</span>
            <div style="display: flex; gap: 8px">
              <button type="button" onClick="{{m.dec}}" aria-label="{{m.decAria}}" style="width: 72px; height: %s; flex: none; border: 0; border-radius: 18px; background: #1f242b; color: #f4f6f8; font: 500 30px Inter, system-ui, sans-serif; cursor: pointer">−</button>
              <button type="button" onClick="{{m.edit}}" aria-label="{{m.editAria}}" style="flex: 1; height: %s; border: 0; border-radius: 18px; background: #0b0d10; color: {{m.valInk}}; font-family: Inter, system-ui, sans-serif; display: flex; align-items: center; justify-content: center; cursor: pointer"><span style="font-size: %s; font-weight: 800; font-variant-numeric: tabular-nums; white-space: nowrap">{{m.value}}</span></button>
              <button type="button" onClick="{{m.inc}}" aria-label="{{m.incAria}}" style="width: 72px; height: %s; flex: none; border: 0; border-radius: 18px; background: #1f242b; color: #f4f6f8; font: 500 30px Inter, system-ui, sans-serif; cursor: pointer">+</button>
            </div>
          </div>
        </sc-for>
"""
    keys = ''.join(["""            <sc-for list="{{keys}}" as="k" hint-placeholder-count="12"><button type="button" onClick="{{k.press}}" aria-label="{{k.aria}}" style="border: 0; border-radius: 18px; background: {{k.bg}}; color: #f4f6f8; font: 700 28px Inter, system-ui, sans-serif; cursor: pointer">{{k.label}}</button></sc-for>
"""])
    body = root_open('tools') + """  <main style="flex: 1; min-width: 0; box-sizing: border-box; padding: 16px; display: flex; flex-direction: column; gap: 10px">
""" + header('Лимиты движения', 'Tools.dc.html', '<sc-if value="{{anyChanged}}" hint-placeholder-val="{{ false }}"><button type="button" onClick="{{resetAll}}" style="height: 64px; padding: 0 18px; border: 0; border-radius: 20px; background: #15181d; color: #5eead4; font: 700 15px Inter, system-ui, sans-serif; cursor: pointer">Сбросить всё</button></sc-if>') + """    <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 10px">
""" + card % ('main', '', '20px', '76px', '76px', '32px', '76px') + """    </div>
    <span style="margin: 4px 4px 0; font-size: 13px; font-weight: 700; letter-spacing: 0.04em; color: #66707b">ТОНКАЯ НАСТРОЙКА</span>
    <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 10px">
""" + card % ('fine', '', '17px', '64px', '64px', '24px', '64px') + """    </div>
    <div style="flex: 1"></div>
    <span style="font-size: 14px; color: #98a1ac">Действует до перезапуска Klipper.</span>
  </main>
  <sc-if value="{{padOpen}}" hint-placeholder-val="{{ false }}">
    <div role="dialog" aria-modal="true" aria-label="Ввести значение" style="position: absolute; inset: 0; background: rgba(5, 6, 8, 0.78); display: flex; align-items: center; justify-content: center">
      <div style="width: 680px; height: 420px; box-sizing: border-box; border-radius: 28px; background: #15181d; padding: 20px; display: grid; grid-template-columns: 240px 1fr; gap: 20px">
        <div style="display: flex; flex-direction: column; gap: 8px">
          <span style="font-size: 15px; font-weight: 600; color: #98a1ac">{{padTitle}}</span>
          <span aria-live="polite" style="height: 96px; border-radius: 20px; background: #0b0d10; display: flex; align-items: center; justify-content: center; font-size: 44px; font-weight: 800; font-variant-numeric: tabular-nums; color: {{entryInk}}">{{entryLabel}}</span>
          <span style="font-size: 14px; color: {{hintInk}}; text-align: center">{{padHint}}</span>
          <div style="flex: 1"></div>
          <button type="button" onClick="{{padOk}}" style="height: 80px; border: 0; border-radius: 22px; background: {{okBg}}; color: {{okFg}}; font: 800 19px Inter, system-ui, sans-serif; cursor: pointer">Готово</button>
          <button type="button" onClick="{{padCancel}}" style="height: 64px; border: 0; border-radius: 20px; background: #1f242b; color: #f4f6f8; font: 600 16px Inter, system-ui, sans-serif; cursor: pointer">Отмена</button>
        </div>
        <div style="display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); grid-template-rows: repeat(4, minmax(0, 1fr)); gap: 8px">
""" + keys + """        </div>
      </div>
    </div>
  </sc-if>
</div>
"""
    body = body.replace('<div style="position: relative; width: 800px;', '<div style="position: relative; width: 800px;', 1)
    js = r"""class Component extends DCLogic {
  constructor(props) {
    super(props);
    this.state = { v: { vel: 600, acc: 20000, scv: 25, mcr: 50 }, pad: null, entry: '' };
  }
  renderVals() {
    var self = this, st = this.state;
    var defs = {
      vel: { title: 'Скорость', hint: 'предел скорости головы', unit: 'мм/с', cfg: 600, min: 10, max: 600, step: 50, dec: 0 },
      acc: { title: 'Ускорение', hint: 'меньше — тише и меньше ряби', unit: 'мм/с²', cfg: 20000, min: 500, max: 20000, step: 1000, dec: 0 },
      scv: { title: 'Скорость в углах', hint: 'выше — быстрее, но углы круглее', unit: 'мм/с', cfg: 25, min: 1, max: 50, step: 1, dec: 0 },
      mcr: { title: 'Плавность разгона', hint: 'сколько пути на полной скорости', unit: '%', cfg: 50, min: 0, max: 100, step: 5, dec: 0 }
    };
    function setv(k, x) { var d = defs[k]; var n = Object.assign({}, self.state.v); n[k] = Math.min(d.max, Math.max(d.min, +x.toFixed(4))); self.setState({ v: n }); }
    function item(k) {
      var d = defs[k], x = st.v[k], changed = x !== d.cfg;
      return {
        title: d.title, hint: d.hint, unit: d.unit, value: d.dec ? x.toFixed(d.dec) : x.toLocaleString('ru-RU'),
        changed: changed, was: 'было ' + (d.dec ? d.cfg.toFixed(d.dec) : d.cfg.toLocaleString('ru-RU')),
        reset: function () { setv(k, d.cfg); }, resetAria: d.title + ': вернуть ' + d.cfg,
        ring: changed ? 'rgba(45, 212, 191, 0.45)' : 'transparent', valInk: changed ? '#5eead4' : '#f4f6f8',
        dec: function () { setv(k, x - d.step); }, inc: function () { setv(k, x + d.step); },
        decAria: d.title + ': меньше', incAria: d.title + ': больше', editAria: d.title + ': ввести число',
        edit: function () { self.setState({ pad: k, entry: '' }); }
      };
    }
    var changedAny = Object.keys(defs).some(function (k) { return st.v[k] !== defs[k].cfg; });
    var pd = st.pad ? defs[st.pad] : null;
    var num = st.entry === '' ? NaN : parseFloat(st.entry.replace(',', '.'));
    var valid = pd && !isNaN(num) && num >= pd.min && num <= pd.max;
    return {
      main: [item('vel'), item('acc')], fine: [item('scv'), item('mcr')],
      anyChanged: changedAny,
      resetAll: function () { self.setState({ v: { vel: 600, acc: 20000, scv: 25, mcr: 50 } }); },
      padOpen: !!st.pad, padTitle: pd ? pd.title + ', ' + pd.unit : '',
      entryLabel: st.entry === '' ? '—' : st.entry, entryInk: st.entry === '' ? '#66707b' : '#f4f6f8',
      padHint: pd ? 'от ' + pd.min + ' до ' + pd.max.toLocaleString('ru-RU') : '', hintInk: (pd && !isNaN(num) && !valid) ? '#fb7185' : '#98a1ac',
      okBg: valid ? '#2dd4bf' : '#1f242b', okFg: valid ? '#042f2a' : '#66707b',
      padOk: function () { if (valid) { setv(st.pad, num); self.setState({ pad: null }); } },
      padCancel: function () { self.setState({ pad: null }); },
      keys: ['1', '2', '3', '4', '5', '6', '7', '8', '9', ',', '0', '⌫'].map(function (k) {
        return { label: k, aria: k === '⌫' ? 'Стереть' : k, bg: k === '⌫' ? '#1f242b' : '#252a31',
          press: function () { var e = self.state.entry; if (k === '⌫') e = e.slice(0, -1); else if (e.length < 7) e = e + k; self.setState({ entry: e }); } };
      })
    };
  }
}"""
    write('Limits', 'Лимиты движения', body, js)
    for lab, src in [('Скорость', 'toolhead.max_velocity; config: configfile.settings.printer.max_velocity'),
                     ('Ускорение', 'toolhead.max_accel; config: configfile.settings.printer.max_accel'),
                     ('Скорость в углах', 'toolhead.square_corner_velocity'),
                     ('Плавность разгона', 'toolhead.minimum_cruise_ratio'),
                     ('apply', 'SET_VELOCITY_LIMIT VELOCITY= ACCEL= SQUARE_CORNER_VELOCITY= MINIMUM_CRUISE_RATIO=')]:
        gen.note_src('Лимиты движения', lab, src)

