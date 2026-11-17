'use strict';
const native = require('./rwikitext.node');
const operations = {1: 'template_text', 2: 'page_exists', 3: 'file_info'};

class WikiText {
  constructor(libraryPath) { this.libraryPath = libraryPath; }
  convert(request, lookup = () => null) {
    const callback = (operation, title) => {
      try {
        const value = lookup(operations[operation], title);
        if (value == null) return [0, null];
        if (typeof value.then === 'function') throw new TypeError('lookup must be synchronous');
        const json = JSON.stringify(value);
        if (json === undefined) throw new TypeError('lookup result must be JSON serializable');
        return [1, json];
      } catch (error) {
        return [2, JSON.stringify(String(error && error.message || error))];
      }
    };
    return JSON.parse(native.convert(this.libraryPath, JSON.stringify(request), callback));
  }
}
module.exports = { WikiText };
