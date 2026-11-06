// The host the PlantUML engine (vendor/plantuml/plantuml.js) expects from a
// browser, in QuickJS (ADR-009): a minimal XML document and serializer for
// writing the SVG, a canvas that measures text with the metrics of DejaVu
// Sans (`__measure`, from Rust), console, and timers on a virtual clock that
// `__step` runs until the render callback was called.
(function () {
  var g = globalThis;
  g.window = g;
  g.self = g;
  g.__out = null;
  g.__err = null;
  var timers = [];
  var clock = 0;
  var next = 1;
  g.setTimeout = function (f, ms) {
    var id = next++;
    timers.push({ id: id, f: f, due: clock + (ms || 0) });
    return id;
  };
  g.clearTimeout = function (id) {
    timers = timers.filter(function (t) { return t.id !== id; });
  };
  var log = function (kind) {
    return function () { __log(kind + ' ' + Array.prototype.join.call(arguments, ' ')); };
  };
  g.console = { log: log('log'), info: log('info'), warn: log('warn'), error: log('error'), debug: function () {} };
  g.navigator = { userAgent: 'underline' };
  g.location = { href: 'file:///' };

  function context() {
    return {
      font: '',
      measureText: function (text) {
        var m = /([\d.]+)px/.exec(this.font);
        var size = m ? parseFloat(m[1]) : 12;
        var width = __measure(String(text), size, /bold/.test(this.font));
        return {
          width: width,
          actualBoundingBoxAscent: size * 0.76,
          actualBoundingBoxDescent: size * 0.24,
          fontBoundingBoxAscent: size * 0.93,
          fontBoundingBoxDescent: size * 0.24
        };
      }
    };
  }

  function escape(text, attribute) {
    text = String(text).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
    return attribute ? text.replace(/"/g, '&quot;') : text;
  }

  function Node(type, name) {
    this.nodeType = type;
    this.nodeName = name;
    this.tagName = name;
    this.childNodes = [];
    this.attrs = [];
    this.style = {};
    this.parentNode = null;
    this.ownerDocument = g.document;
    this._text = '';
  }
  Node.prototype.setAttribute = function (k, v) {
    for (var i = 0; i < this.attrs.length; i++) {
      if (this.attrs[i][0] === k) { this.attrs[i][1] = String(v); return; }
    }
    this.attrs.push([k, String(v)]);
  };
  Node.prototype.setAttributeNS = function (ns, k, v) { this.setAttribute(k, v); };
  Node.prototype.getAttribute = function (k) {
    for (var i = 0; i < this.attrs.length; i++) if (this.attrs[i][0] === k) return this.attrs[i][1];
    return null;
  };
  Node.prototype.appendChild = function (c) {
    if (c.parentNode) c.parentNode.removeChild(c);
    c.parentNode = this;
    this.childNodes.push(c);
    return c;
  };
  Node.prototype.insertBefore = function (c, ref) {
    if (c.parentNode) c.parentNode.removeChild(c);
    c.parentNode = this;
    var i = this.childNodes.indexOf(ref);
    if (i < 0) this.childNodes.push(c); else this.childNodes.splice(i, 0, c);
    return c;
  };
  Node.prototype.removeChild = function (c) {
    var i = this.childNodes.indexOf(c);
    if (i >= 0) this.childNodes.splice(i, 1);
    c.parentNode = null;
    return c;
  };
  Object.defineProperty(Node.prototype, 'firstChild', {
    get: function () { return this.childNodes[0] || null; }
  });
  Object.defineProperty(Node.prototype, 'textContent', {
    get: function () {
      if (this.nodeType === 3 || this.nodeType === 7) return this._text;
      return this.childNodes.map(function (c) { return c.textContent; }).join('');
    },
    set: function (v) {
      if (this.nodeType === 3 || this.nodeType === 7) { this._text = String(v); return; }
      this.childNodes = [];
      var t = new Node(3, '#text');
      t._text = String(v);
      this.appendChild(t);
    }
  });
  Node.prototype.getBBox = function () {
    var size = parseFloat(this.getAttribute('font-size') || 14);
    var bold = /bold/.test(this.getAttribute('font-weight') || '');
    return { x: 0, y: -size * 0.8, width: __measure(this.textContent, size, bold), height: size * 1.2 };
  };
  Node.prototype.getContext = function () { return context(); };

  function serialize(n) {
    if (n.nodeType === 3) return escape(n._text, false);
    if (n.nodeType === 7) return '<?' + n.nodeName + ' ' + n._text + '?>';
    var out = '<' + n.nodeName;
    if (n.nodeName === 'svg' && !n.getAttribute('xmlns')) {
      out += ' xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"';
    }
    for (var i = 0; i < n.attrs.length; i++) out += ' ' + n.attrs[i][0] + '="' + escape(n.attrs[i][1], true) + '"';
    if (!n.childNodes.length) return out + '/>';
    return out + '>' + n.childNodes.map(serialize).join('') + '</' + n.nodeName + '>';
  }
  g.XMLSerializer = function () {};
  g.XMLSerializer.prototype.serializeToString = serialize;

  g.document = {
    createElement: function (tag) { return new Node(1, tag); },
    createElementNS: function (ns, tag) { return new Node(1, tag); },
    createTextNode: function (t) { var n = new Node(3, '#text'); n._text = String(t); return n; },
    createProcessingInstruction: function (target, data) {
      var n = new Node(7, target);
      n._text = String(data);
      return n;
    },
    getElementById: function () { return null; }
  };
  g.document.body = new Node(1, 'body');
  g.document.head = new Node(1, 'head');

  // Runs the next timer; false when the render is done or nothing is left.
  g.__step = function () {
    if (g.__out !== null || g.__err !== null || !timers.length) return false;
    var best = 0;
    for (var i = 1; i < timers.length; i++) if (timers[i].due < timers[best].due) best = i;
    var t = timers.splice(best, 1)[0];
    if (t.due > clock) clock = t.due;
    t.f();
    return true;
  };
})();
