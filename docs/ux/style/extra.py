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
