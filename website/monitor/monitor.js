/* SOLBEAM — /monitor/
 *
 * No build step, no framework, no dependencies, no API keys, no server.
 *
 * Every figure the page shows is one of:
 *   live          — fetched by the reader's browser from a public, keyless API
 *   derived       — computed from a live figure plus an assumption from data.json
 *   cached        — the last good value, with its age, when a source fails
 *   unavailable   — the source failed and there is no cached value
 *   NOT PUBLISHED — a placeholder for a hand-published value (null in data.json)
 *
 * A failed fetch never renders as 0 and never as a dash. "We could not read it"
 * and "it is zero" are different statements.
 */
(function () {
  'use strict';

  var TIMEOUT_MS = 9000;
  var CACHE_PREFIX = 'solbeam.monitor.v1.';
  var DATA_JSON = '/monitor/data.json';

  var API = {
    chain:   'https://api.whatsonchain.com/v1/bsv/main/chain/info',
    headers: 'https://api.whatsonchain.com/v1/bsv/main/block/headers?count=10',
    cg:      'https://api.coingecko.com/api/v3/simple/price?ids=bitcoin-cash-sv,solana&vs_currencies=usd&include_last_updated_at=true',
    cbBsv:   'https://api.coinbase.com/v2/prices/BSV-USD/spot',
    cbSol:   'https://api.coinbase.com/v2/prices/SOL-USD/spot',
    solRpc:  'https://solana-rpc.publicnode.com'
  };

  /* The leased-hashrate inputs are published snapshots, not a live API call:
     the page reads the rate and the rentable capacity from data.json, whose
     fields cite crypto51.app and the time they were read. These URLs are the
     citations shown on the page. */
  var LEASE_SRC = 'https://api.crypto51.app/coins.json';

  /* Fallback source links, used when a figure goes "unavailable" and the live
     meta (which carries the source) was never written. */
  var SOURCES = {
    'bsv-height':   ['WhatsOnChain chain/info', API.chain],
    'bsv-hashrate': ['WhatsOnChain chain/info (difficulty)', API.chain],
    'bsv-spacing':  ['WhatsOnChain block/headers', API.headers],
    'bsv-price':    ['CoinGecko simple/price, or Coinbase spot', API.cg],
    'sol-price':    ['CoinGecko simple/price, or Coinbase spot', API.cg],
    'sol-stake':    ['Solana RPC getVoteAccounts', API.solRpc],
    'sol-tps':      ['Solana RPC getRecentPerformanceSamples', API.solRpc],
    'lease':        ['crypto51.app NiceHash SHA-256 rental prices', LEASE_SRC]
  };

  /* Live figures this run has collected, for the derived cards. */
  var V = {
    dataLoaded: false,
    assumptions: null,
    reserve: null,
    supply: null,
    reserveAsOf: null,
    hashrateH: null,
    difficulty: null,
    spacingMean: null,
    bsvPrice: null
  };

  var runState = { total: 0, live: 0 };

  /* ---------------------------------------------------------------- format */

  function pad(n) { return (n < 10 ? '0' : '') + n; }

  function utc(ms) {
    if (ms == null || !isFinite(ms)) return 'unknown time';
    var d = new Date(ms);
    return d.getUTCFullYear() + '-' + pad(d.getUTCMonth() + 1) + '-' + pad(d.getUTCDate()) + ' ' +
           pad(d.getUTCHours()) + ':' + pad(d.getUTCMinutes()) + ' UTC';
  }

  function clock() {
    var d = new Date();
    return pad(d.getUTCHours()) + ':' + pad(d.getUTCMinutes()) + ' UTC';
  }

  function age(ms) {
    var s = Math.max(0, Math.round((Date.now() - ms) / 1000));
    if (s < 60) return s + ' s';
    var m = Math.round(s / 60);
    if (m < 60) return m + ' min';
    var h = Math.floor(m / 60), rm = m % 60;
    if (h < 48) return h + ' h' + (rm ? ' ' + rm + ' min' : '');
    return Math.round(h / 24) + ' days';
  }

  var NF = new Intl.NumberFormat('en-US');

  function dec(n, d) {
    return new Intl.NumberFormat('en-US', {
      minimumFractionDigits: d, maximumFractionDigits: d
    }).format(n);
  }

  function money(n) {
    if (!isFinite(n)) return null;
    if (n >= 1e9) return '$' + dec(n / 1e9, 2) + 'B';
    if (n >= 1e6) return '$' + dec(n / 1e6, 2) + 'M';
    if (n >= 1e3) return '$' + NF.format(Math.round(n));
    return '$' + dec(n, 2);
  }

  function humanHash(h) {
    if (h >= 1e18) return { v: h / 1e18, u: 'EH/s' };
    if (h >= 1e15) return { v: h / 1e15, u: 'PH/s' };
    if (h >= 1e12) return { v: h / 1e12, u: 'TH/s' };
    return { v: h, u: 'H/s' };
  }

  function hashText(h) {
    var x = humanHash(h);
    return dec(x.v, x.v < 10 ? 2 : 1) + ' ' + x.u;
  }

  /* Rentable capacity is published in whole PH/s, so print it as published. */
  function phText(ph) {
    return dec(ph, (Math.abs(ph - Math.round(ph)) < 1e-9) ? 0 : 2) + ' PH/s';
  }

  function powerText(kw) {
    if (kw >= 1000) return dec(kw / 1000, 2) + ' MW';
    return dec(kw, 1) + ' kW';
  }

  function esc(s) {
    return String(s).replace(/[&<>"']/g, function (c) {
      return { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c];
    });
  }

  function src(name, url) {
    return 'Source: <a href="' + esc(url) + '" target="_blank" rel="noopener">' + esc(name) + '</a>';
  }

  function numOrNull(x) {
    return (typeof x === 'number' && isFinite(x)) ? x : null;
  }

  function elapsed(start) {
    return dec((Date.now() - start) / 1000, 1) + ' s';
  }

  /* ---------------------------------------------------------------- render */

  function el(id) { return document.getElementById(id); }

  function paint(key, p) {
    var f = el('fig-' + key), s = el('state-' + key), m = el('meta-' + key);
    if (!f) return;
    f.textContent = p.text;
    if (s) {
      s.textContent = p.state || '';
      s.className = 'mon-card__state ' + (p.stateClass || '');
    }
    if (m) m.innerHTML = p.meta || '';
    if (p.cache !== false) {
      try {
        localStorage.setItem(CACHE_PREFIX + key, JSON.stringify({
          text: p.text, meta: p.meta, state: p.state, stateClass: p.stateClass, at: Date.now()
        }));
      } catch (e) { /* private mode / quota — caching is a nicety, not required */ }
    }
  }

  function cacheGet(key) {
    try { return JSON.parse(localStorage.getItem(CACHE_PREFIX + key)); } catch (e) { return null; }
  }

  function setRaw(k, v) {
    try { localStorage.setItem(CACHE_PREFIX + 'raw.' + k, JSON.stringify({ v: v, at: Date.now() })); } catch (e) {}
  }

  function getRaw(k) {
    try {
      var o = JSON.parse(localStorage.getItem(CACHE_PREFIX + 'raw.' + k));
      return (o && typeof o.v === 'number') ? o.v : null;
    } catch (e) { return null; }
  }

  function errMsg(e) {
    return (e && e.message) ? e.message : 'failed';
  }

  function paintDown(key, reason) {
    var c = cacheGet(key);
    var s = SOURCES[key];
    if (c && c.text) {
      paint(key, {
        text: c.text,
        state: 'cached \u00b7 ' + age(c.at),
        stateClass: 'is-cached',
        cache: false,
        meta: 'Last good value, read ' + utc(c.at) + ' (' + age(c.at) + ' old). This time the ' +
              'source failed: ' + esc(reason) + '. Detail from that read — ' + (c.meta || '')
      });
    } else {
      paint(key, {
        text: 'unavailable',
        state: 'no data',
        stateClass: 'is-down',
        cache: false,
        meta: 'The source did not respond (' + esc(reason) + ') and there is no cached value, so ' +
              'there is no number to show. ' + (s ? src(s[0], s[1]) : '')
      });
    }
  }

  function run(keys, fn) {
    var t0 = Date.now();
    keys.forEach(function (k) { runState.total++; });
    return fn(t0).then(function (map) {
      keys.forEach(function (k) {
        var p = map && map[k];
        if (p) { paint(k, p); runState.live++; }
        else { paintDown(k, 'no value returned'); }
      });
    }, function (err) {
      keys.forEach(function (k) { paintDown(k, errMsg(err)); });
    });
  }

  /* ----------------------------------------------------------------- fetch */

  function fetchJSON(url, opts) {
    var o = { cache: 'no-store' };
    if (opts) {
      for (var k in opts) {
        if (Object.prototype.hasOwnProperty.call(opts, k)) o[k] = opts[k];
      }
    }
    var ctrl = (typeof AbortController !== 'undefined') ? new AbortController() : null;
    var timer = setTimeout(function () { if (ctrl) ctrl.abort(); }, TIMEOUT_MS);
    if (ctrl) o.signal = ctrl.signal;
    return fetch(url, o).then(function (r) {
      if (!r.ok) throw new Error('HTTP ' + r.status);
      return r.json();
    }).then(function (j) {
      clearTimeout(timer);
      return j;
    }, function (e) {
      clearTimeout(timer);
      throw (e && e.name === 'AbortError') ? new Error('timed out after ' + (TIMEOUT_MS / 1000) + ' s') : e;
    });
  }

  function rpc(method, params) {
    return fetchJSON(API.solRpc, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ jsonrpc: '2.0', id: 1, method: method, params: params || [] })
    }).then(function (j) {
      if (j && j.error) throw new Error('RPC ' + (j.error.code || '') + (j.error.message ? ': ' + j.error.message : ''));
      return j.result;
    });
  }

  function targetSeconds() {
    var a = V.assumptions;
    var t = a ? numOrNull(a.bsv_target_block_seconds) : null;
    return (t && t > 0) ? t : 600;
  }

  /* --- BSV chain info: block height + estimated hashrate ------------------ */

  function fetchChain(t0) {
    return fetchJSON(API.chain).then(function (j) {
      var height = numOrNull(j.blocks);
      var diff = numOrNull(j.difficulty);
      if (height == null || diff == null) throw new Error('unexpected chain/info shape');
      var target = targetSeconds();
      var hp = diff * Math.pow(2, 32) / target;
      V.hashrateH = hp;
      V.difficulty = diff;
      setRaw('hashrateH', hp);
      var srcTime = numOrNull(j.mediantime) != null ? j.mediantime * 1000 : Date.now();
      var checked = 'checked ' + clock() + ' (' + elapsed(t0) + ')';
      return {
        'bsv-height': {
          text: NF.format(height),
          state: 'live',
          stateClass: 'is-live',
          meta: src('WhatsOnChain chain/info', API.chain) + ' \u00b7 block ' + NF.format(height) +
                ' \u00b7 median time ' + utc(srcTime) + ' \u00b7 ' + checked
        },
        'bsv-hashrate': {
          text: hashText(hp),
          state: 'live \u00b7 estimated',
          stateClass: 'is-live',
          meta: src('WhatsOnChain chain/info', API.chain) + ' \u00b7 difficulty ' +
                NF.format(Math.round(diff)) + ' \u00d7 2\u00b3\u00b2 \u00f7 ' + dec(target, 0) +
                ' s target. The estimate assumes blocks arrive at the target; the observed spacing is ' +
                'published separately. ' + checked
        }
      };
    });
  }

  /* --- BSV observed block spacing ---------------------------------------- */

  function fetchSpacing(t0) {
    return fetchJSON(API.headers).then(function (list) {
      if (!list || list.length < 2) throw new Error('fewer than 2 headers returned');
      list = list.slice().sort(function (a, b) { return a.height - b.height; });
      var times = list.map(function (x) { return x.time; });
      var gaps = [];
      for (var i = 1; i < times.length; i++) gaps.push(times[i] - times[i - 1]);
      if (!gaps.length) throw new Error('no usable header timestamps');
      var mean = gaps.reduce(function (a, b) { return a + b; }, 0) / gaps.length;
      var sorted = gaps.slice().sort(function (a, b) { return a - b; });
      var mid = sorted.length % 2
        ? sorted[(sorted.length - 1) / 2]
        : (sorted[sorted.length / 2 - 1] + sorted[sorted.length / 2]) / 2;
      V.spacingMean = mean;
      var newest = list[list.length - 1].time * 1000;
      return {
        'bsv-spacing': {
          text: Math.round(mean) + ' s mean',
          state: 'live',
          stateClass: 'is-live',
          meta: 'Mean over the last ' + gaps.length + ' intervals between the ' + list.length +
                ' newest headers (the API returns at most 10). Median ' + Math.round(mid) +
                ' s; target 600 s. Newest header ' + utc(newest) + '. ' +
                src('WhatsOnChain block/headers', API.headers) + ' \u00b7 checked ' + clock() +
                ' (' + elapsed(t0) + ')'
        }
      };
    });
  }

  /* --- prices: CoinGecko, falling back to Coinbase per asset -------------- */

  function coinbase(asset, url) {
    return fetchJSON(url).then(function (j) {
      var amt = (j && j.data) ? parseFloat(j.data.amount) : NaN;
      if (!isFinite(amt)) throw new Error('unexpected Coinbase shape');
      return { usd: amt, t: Date.now(), name: 'Coinbase spot ' + asset + '-USD', url: url, requestTime: true };
    });
  }

  function fromCoinGecko(j, id, label) {
    var o = j && j[id];
    var usd = o ? numOrNull(o.usd) : null;
    if (usd == null) return null;
    return {
      usd: usd,
      t: numOrNull(o.last_updated_at) != null ? o.last_updated_at * 1000 : Date.now(),
      name: 'CoinGecko simple/price',
      url: 'https://www.coingecko.com/en/coins/' + label,
      requestTime: numOrNull(o.last_updated_at) == null
    };
  }

  function priceMap(bsv, sol, t0) {
    var out = {};
    var checked = 'checked ' + clock() + ' (' + elapsed(t0) + ')';
    if (bsv && isFinite(bsv.usd)) {
      V.bsvPrice = bsv.usd;
      setRaw('bsvPrice', bsv.usd);
      out['bsv-price'] = {
        text: '$' + dec(bsv.usd, bsv.usd < 1 ? 4 : 2),
        state: 'live',
        stateClass: 'is-live',
        meta: src(bsv.name, bsv.url) + ' \u00b7 ' +
              (bsv.requestTime ? 'spot at request time' : 'source updated ' + utc(bsv.t)) + ' \u00b7 ' + checked
      };
    }
    if (sol && isFinite(sol.usd)) {
      out['sol-price'] = {
        text: '$' + dec(sol.usd, sol.usd < 1 ? 4 : 2),
        state: 'live',
        stateClass: 'is-live',
        meta: src(sol.name, sol.url) + ' \u00b7 ' +
              (sol.requestTime ? 'spot at request time' : 'source updated ' + utc(sol.t)) + ' \u00b7 ' + checked
      };
    }
    return out;
  }

  function fetchPrices(t0) {
    return fetchJSON(API.cg).then(function (j) {
      return j;
    }, function () {
      return null; /* CoinGecko down or rate-limited; Coinbase below still counts as live */
    }).then(function (j) {
      var bsv = fromCoinGecko(j, 'bitcoin-cash-sv', 'bitcoin-cash-sv');
      var sol = fromCoinGecko(j, 'solana', 'solana');
      return Promise.all([
        bsv ? Promise.resolve(bsv) : coinbase('BSV', API.cbBsv).catch(function () { return null; }),
        sol ? Promise.resolve(sol) : coinbase('SOL', API.cbSol).catch(function () { return null; })
      ]);
    }).then(function (p) {
      return priceMap(p[0], p[1], t0);
    });
  }

  /* --- Solana: active stake, then throughput ------------------------------ */

  function fetchStake(t0) {
    return rpc('getVoteAccounts', [{ keepUnstakedDelinquents: false, delinquentSlotDistance: 2 }])
      .then(function (res) {
        var cur = (res && res.current) || [];
        var del = (res && res.delinquent) || [];
        var lamports = cur.reduce(function (a, v) { return a + (numOrNull(v.activatedStake) || 0); }, 0);
        var sol = lamports / 1e9;
        if (!(sol > 0)) throw new Error('no active stake returned');
        return {
          'sol-stake': {
            text: dec(sol / 1e6, 2) + 'M SOL',
            state: 'live \u00b7 proof-of-stake',
            stateClass: 'is-live',
            meta: NF.format(Math.round(sol)) + ' SOL of active stake across ' + NF.format(cur.length) +
                  ' vote accounts (' + NF.format(del.length) + ' delinquent). This is Solana\u2019s security ' +
                  'figure \u2014 there is no hashrate. ' + src('Solana RPC getVoteAccounts', API.solRpc) +
                  ' \u00b7 reply ' + clock() + ' (' + elapsed(t0) + ')'
          }
        };
      });
  }

  function fetchTps(t0) {
    return rpc('getRecentPerformanceSamples', [1]).then(function (samples) {
      var s = samples && samples[0];
      if (!s || !(numOrNull(s.samplePeriodSecs) > 0)) throw new Error('no performance sample');
      var total = s.numTransactions / s.samplePeriodSecs;
      var nonVote = s.numNonVoteTransactions / s.samplePeriodSecs;
      return {
        'sol-tps': {
          text: NF.format(Math.round(total)) + ' TPS',
          state: 'live',
          stateClass: 'is-live',
          meta: NF.format(Math.round(nonVote)) + ' TPS excluding vote transactions, over the latest ' +
                dec(s.samplePeriodSecs, 0) + ' s sample ending at slot ' + NF.format(s.slot) + '. ' +
                src('Solana RPC getRecentPerformanceSamples', API.solRpc) + ' \u00b7 reply ' + clock() +
                ' (' + elapsed(t0) + ')'
        }
      };
    });
  }

  /* --- hand-published values ---------------------------------------------- */

  function setText(id, t) { var e = el(id); if (e) e.textContent = t; }

  function publishedWhen(asOf, publishedAt) {
    var t = asOf ? Date.parse(asOf) : NaN;
    if (!isFinite(t)) t = publishedAt ? Date.parse(publishedAt) : NaN;
    if (!isFinite(t)) return { state: 'published', cls: 'is-live' };
    var days = (Date.now() - t) / 86400000;
    if (days > 30) return { state: 'published \u00b7 ' + Math.round(days) + ' days old', cls: 'is-cached' };
    return { state: 'published \u00b7 ' + utc(t), cls: 'is-live' };
  }

  function paintAssumptions() {
    var a = V.assumptions;
    var ids = ['a-multiple', 'a-machine', 'a-mh', 'a-power', 'a-energy', 'a-lease-rate',
               'a-rentable', 'a-target'];
    if (!a) {
      ids.forEach(function (id) {
        setText(id, V.dataLoaded ? 'missing from data.json' : 'not loaded');
      });
      return;
    }
    var mult = numOrNull(a.hashrate_multiple);
    setText('a-multiple', dec(mult == null ? 1 : mult, 2) + ' \u00d7 the network hashrate estimate');
    setText('a-machine', a.machine || 'unspecified in data.json');
    var mh = numOrNull(a.machine_hashrate_ths);
    setText('a-mh', (mh == null ? 'unset' : dec(mh, 0)) + ' TH/s per machine');
    var pw = numOrNull(a.machine_power_kw);
    setText('a-power', (pw == null ? 'unset' : dec(pw, 2)) + ' kW per machine');
    var en = numOrNull(a.energy_usd_per_kwh);
    setText('a-energy', (en == null ? 'unset' : '$' + dec(en, 2)) + ' per kWh (assumed)');
    var lr = numOrNull(a.leased_rate_usd_per_eh_hour);
    setText('a-lease-rate', lr == null
      ? 'not published in data.json'
      : '$' + dec(lr, 2) + ' per EH/s per hour (published)');
    var rc = numOrNull(a.rentable_capacity_ph_s);
    setText('a-rentable', rc == null
      ? 'not published in data.json'
      : phText(rc) + ' listed for rent, whole market (published)');
    var tg = numOrNull(a.bsv_target_block_seconds);
    setText('a-target', (tg == null ? '600' : dec(tg, 0)) + ' s');
  }

  function paintPublished(j) {
    j = j || {};
    var atRaw = j.published_at || null;
    var at = atRaw ? Date.parse(atRaw) : NaN;
    setText('published-at', isFinite(at)
      ? utc(at)
      : 'never \u2014 no reserve or supply figure has been published yet');

    if (V.reserve == null) {
      paint('reserve', {
        text: 'NOT PUBLISHED', state: 'placeholder', stateClass: 'is-none', cache: false,
        meta: 'The BSV under the federation\u2019s 2-of-2 reserve script. It is off-chain, so no public ' +
              'API can read it. It will be published here by hand, from the reserve addresses, once a ' +
              'federation holds a reserve on a public network \u2014 the federation does not exist yet.'
      });
    } else {
      var st = publishedWhen(j.reserve && j.reserve.as_of, atRaw);
      paint('reserve', {
        text: NF.format(V.reserve) + ' BSV', state: st.state, stateClass: st.cls, cache: false,
        meta: 'Hand-published in ' + src('data.json', DATA_JSON) +
              (j.reserve && j.reserve.as_of ? ' \u00b7 as of ' + esc(String(j.reserve.as_of)) : '') +
              '. Off-chain, so nobody can verify it from a public API \u2014 it is published so it can be ' +
              'checked against the reserve addresses.'
      });
    }

    if (V.supply == null) {
      paint('supply', {
        text: 'NOT PUBLISHED', state: 'placeholder', stateClass: 'is-none', cache: false,
        meta: 'Outstanding solBSV. It is on-chain, but the program runs only on a local ' +
              'solana-test-validator, so there is no public mint to read. It will be published here ' +
              'when the program is on a public network.'
      });
    } else {
      var ss = publishedWhen(j.supply && j.supply.as_of, atRaw);
      paint('supply', {
        text: NF.format(V.supply) + ' solBSV', state: ss.state, stateClass: ss.cls, cache: false,
        meta: 'Hand-published in ' + src('data.json', DATA_JSON) +
              (j.supply && j.supply.as_of ? ' \u00b7 as of ' + esc(String(j.supply.as_of)) : '') + '.'
      });
    }

    paintAssumptions();
  }

  function loadData() {
    return fetchJSON(DATA_JSON).then(function (j) {
      V.dataLoaded = true;
      V.assumptions = j.attack_cost_assumptions || null;
      V.reserve = numOrNull(j.reserve && j.reserve.bsv_held);
      V.supply = numOrNull(j.supply && j.supply.solbsv_minted);
      return j;
    });
  }

  /* --- derived: ratio, attack cost, implied hashrate ----------------------- */

  /* Two independent bases, so a missing input on one does not blank the other:
       elec     — electricity-only floor from an assumed machine and energy price
       leased   — published SHA-256 market rate applied to the same hashrate
       rentable — the availability constraint: live network vs the whole market
     `needH` is the attacker's hashrate (network estimate x multiple); at the
     default multiple of 1.0 it is the live network estimate itself. The
     availability ratio uses the raw network hashrate, not the attacker figure. */
  function computeAttack(hp, a) {
    if (!a) return null;
    var mult = numOrNull(a.hashrate_multiple);
    if (mult == null) mult = 1;
    var needH = hp * mult;
    var out = { mult: mult, networkH: hp, needH: needH, elec: null, leased: null, rentable: null };

    var mh = numOrNull(a.machine_hashrate_ths);
    var pw = numOrNull(a.machine_power_kw);
    var en = numOrNull(a.energy_usd_per_kwh);
    if ((mh > 0) && (pw > 0) && (en > 0)) {
      var machines = needH / (mh * 1e12);
      var kw = machines * pw;
      var perHour = kw * en;
      out.elec = { machines: machines, kw: kw, perHour: perHour, perDay: perHour * 24 };
    }

    var rate = numOrNull(a.leased_rate_usd_per_eh_hour);
    if (rate > 0) {
      var leasedPerHour = (needH / 1e18) * rate;
      out.leased = { rate: rate, perHour: leasedPerHour, perDay: leasedPerHour * 24 };
    }

    var rentablePh = numOrNull(a.rentable_capacity_ph_s);
    if (rentablePh > 0) {
      var rentableH = rentablePh * 1e15;
      out.rentable = {
        ph: rentablePh,
        h: rentableH,
        multiple: hp / rentableH,         /* network hashrate / rentable capacity */
        pct: (rentableH / needH) * 100    /* rentable as a share of what the attack needs */
      };
    }
    return out;
  }

  /* The availability constraint: the whole rentable SHA-256 market against the
     hashrate the attack needs. The ratio is computed from the LIVE network
     hashrate and the published rentable capacity, never hard-coded. `hp` and
     `rentable` are passed separately so the placeholder names the missing
     input instead of blaming the wrong source. */
  function paintAvailability(hp, rentable, fromCache) {
    var box = el('av-figures');
    if (!box) return;
    setText('d-rentablemultiple', 'not derived');

    if (hp == null) {
      box.innerHTML = 'SHA-256 rental markets list a published amount of capacity against BSV\u2019s live ' +
        'network hashrate. The live hashrate could not be read, so the comparison cannot be computed ' +
        'right now \u2014 this is a placeholder, not a zero.';
      return;
    }
    if (!rentable) {
      box.innerHTML = 'SHA-256 rental markets list a published amount of capacity, but ' +
        '<a href="/monitor/data.json">data.json</a> has no usable rentable_capacity_ph_s, so the ' +
        'comparison against the live ' + esc(hashText(hp)) + ' network is a labelled placeholder ' +
        '\u2014 not a zero and not a guess.';
      return;
    }

    box.innerHTML = 'SHA-256 rental markets list roughly <strong class="av-num">' + esc(phText(rentable.ph)) +
      '</strong> of capacity (the whole market, not BSV\u2019s share) against BSV\u2019s ' +
      '<strong class="av-num">' + esc(hashText(hp)) + '</strong> ' +
      (fromCache ? 'last-read' : 'live') + ' network \u2014 the network is ' +
      '<strong class="av-num">' + esc(NF.format(Math.round(rentable.multiple))) + '\u00d7</strong> the ' +
      'rentable supply, so the entire rentable supply is <strong class="av-num">' +
      esc(dec(rentable.pct, 2)) + '%</strong> of what the attack needs.';
    setText('d-rentablemultiple',
      NF.format(Math.round(rentable.multiple)) + '\u00d7 (network \u00f7 rentable)');
  }

  function paintDerived() {
    /* Reserve-to-supply ratio. */
    if (V.reserve != null && V.supply != null && V.supply > 0) {
      var r = V.reserve / V.supply;
      paint('ratio', {
        text: dec(r, 2) + '\u00d7',
        state: r >= 1 ? 'backed \u00b7 ' + dec(r * 100, 1) + '%' : 'UNDER-COLLATERALISED',
        stateClass: r >= 1 ? 'is-live' : 'is-down',
        cache: false,
        meta: NF.format(V.reserve) + ' BSV held \u00f7 ' + NF.format(V.supply) + ' solBSV outstanding = ' +
              dec(r, 3) + '\u00d7. ' + src('data.json', DATA_JSON) +
              '. The protocol cannot check this; it is published so a reader can hold it to account.'
      });
    } else {
      var why = V.dataLoaded
        ? 'Both figures are still placeholders, so there is nothing to divide.'
        : 'data.json could not be read, so the hand-published figures are unknown.';
      paint('ratio', {
        text: 'NOT PUBLISHED', state: 'placeholder', stateClass: 'is-none', cache: false,
        meta: 'Custodied BSV \u00f7 outstanding solBSV. No public API can produce this \u2014 the reserve ' +
              'is off-chain BSV. ' + why + ' It appears here the moment both numbers in ' +
              '<a href="/monitor/data.json">data.json</a> are filled in.'
      });
    }

    /* Cost to out-mine the chain, on two bases: the electricity-only floor and
       the leased-hashrate (market) rate. They are computed independently, so a
       missing energy price does not blank the leased figure, and vice versa. */
    var hp = (V.hashrateH != null) ? V.hashrateH : getRaw('hashrateH');
    var fromCache = (V.hashrateH == null) && (getRaw('hashrateH') != null);
    var calc = (hp != null) ? computeAttack(hp, V.assumptions) : null;
    var bp = (V.bsvPrice != null) ? V.bsvPrice : getRaw('bsvPrice');
    var basis = (fromCache ? 'last read' : 'live') + ' network estimate of ' + hashText(hp);
    var elecRowIds = ['d-needhash', 'd-machines', 'd-power', 'd-perhour', 'd-perday', 'd-bsvday'];
    var leaseRowIds = ['d-leasehour', 'd-leaseday', 'd-rentablemultiple'];

    if (hp == null) {
      paint('attack', {
        text: 'unavailable', state: 'needs live hashrate', stateClass: 'is-down', cache: false,
        meta: 'The cost is derived from the live BSV hashrate, which could not be read and has no ' +
              'cached value, so there is no honest number to show. The assumptions below are still ' +
              'printed, and the source is ' + src('WhatsOnChain chain/info', API.chain) + '.'
      });
      paint('lease', {
        text: 'unavailable', state: 'needs live hashrate', stateClass: 'is-down', cache: false,
        meta: 'The leased cost is the published market rate applied to the live BSV hashrate. The ' +
              'hashrate could not be read and has no cached value, so the rate cannot be turned into ' +
              'a cost \u2014 this is a placeholder, not a zero. Rate source: ' +
              src('crypto51.app (NiceHash SHA-256 prices)', LEASE_SRC) + '.'
      });
      elecRowIds.concat(leaseRowIds).forEach(function (id) { setText(id, 'not derived'); });
      paintAvailability(null, null, false);
    } else if (!calc) {
      paint('attack', {
        text: 'unavailable', state: 'assumptions not loaded', stateClass: 'is-down', cache: false,
        meta: 'The live hashrate was read, but <a href="/monitor/data.json">data.json</a> could not be ' +
              'read (or has no attack_cost_assumptions block), so neither cost basis can be computed. ' +
              'This is a placeholder, not a zero.'
      });
      paint('lease', {
        text: 'NOT PUBLISHED', state: 'rate not published', stateClass: 'is-none', cache: false,
        meta: 'No usable leased_rate_usd_per_eh_hour was read from ' +
              '<a href="/monitor/data.json">data.json</a>, so there is no sourced rate to apply. This ' +
              'is a placeholder, not a guess. The rate, when published, is the NiceHash SHA-256 price ' +
              'via ' + src('crypto51.app', LEASE_SRC) + '.'
      });
      elecRowIds.concat(leaseRowIds).forEach(function (id) { setText(id, 'not derived'); });
      paintAvailability(hp, null, fromCache);
    } else {
      /* Electricity floor — unchanged. */
      if (calc.elec) {
        var bsvPerDay = (bp != null && bp > 0) ? calc.elec.perDay / bp : null;
        var meta = money(calc.elec.perHour) + '/hour of electricity for ' + hashText(calc.needH) + ' (' +
                   dec(calc.mult, 2) + '\u00d7 the ' + basis + '), across ' +
                   NF.format(Math.round(calc.elec.machines)) + ' assumed machines drawing ' +
                   powerText(calc.elec.kw) + '.';
        if (bsvPerDay != null) meta += ' At the live BSV price that is ' + dec(bsvPerDay, 1) + ' BSV/day.';
        meta += ' Assumptions, with units, are tabulated below \u2014 they are ours, not measurements.';
        paint('attack', {
          text: money(calc.elec.perDay) + '/day',
          state: fromCache ? 'derived \u00b7 hashrate cached' : 'derived \u00b7 live hashrate',
          stateClass: fromCache ? 'is-cached' : 'is-derived',
          cache: false,
          meta: meta
        });
        setText('d-needhash', hashText(calc.needH) + ' (' + dec(calc.mult, 2) + '\u00d7 live network estimate)');
        setText('d-machines', NF.format(Math.round(calc.elec.machines)));
        setText('d-power', powerText(calc.elec.kw));
        setText('d-perhour', money(calc.elec.perHour));
        setText('d-perday', money(calc.elec.perDay));
        setText('d-bsvday', bsvPerDay != null ? dec(bsvPerDay, 1) + ' BSV' : 'needs a live BSV price');
      } else {
        paint('attack', {
          text: 'unavailable', state: 'assumption missing', stateClass: 'is-down', cache: false,
          meta: 'The live hashrate was read, but data.json did not provide a usable hardware assumption ' +
                '(machine hash rate, power draw and energy price must all be present and positive), so no ' +
                'electricity cost can be derived. See <a href="/monitor/data.json">data.json</a>.'
        });
        elecRowIds.forEach(function (id) { setText(id, 'not derived'); });
      }

      /* Leased-hashrate cost: the published market rate on the same attacker
         hashrate. At the default multiple of 1.0 this is BSV's live hashrate. */
      if (calc.leased) {
        var L = calc.leased;
        paint('lease', {
          text: money(L.perHour) + '/hour',
          state: fromCache ? 'leased rate \u00b7 hashrate cached' : 'leased rate \u00b7 live hashrate',
          stateClass: fromCache ? 'is-cached' : 'is-derived',
          cache: false,
          meta: '$' + dec(L.rate, 2) + ' per EH/s per hour \u00d7 ' + hashText(calc.needH) + ' (' +
                dec(calc.mult, 2) + '\u00d7 the ' + basis + ') = ' + money(L.perHour) + '/hour, ' +
                money(L.perDay) + '/day. ' + src('crypto51.app (NiceHash SHA-256 prices)', LEASE_SRC) +
                '. A rate, not an offer: the market lists a fraction of this hashrate \u2014 see the ' +
                'availability note above. Excludes buying the hardware.'
        });
        setText('d-leasehour', money(L.perHour));
        setText('d-leaseday', money(L.perDay));
      } else {
        paint('lease', {
          text: 'NOT PUBLISHED', state: 'rate not published', stateClass: 'is-none', cache: false,
          meta: 'No usable leased_rate_usd_per_eh_hour was read from ' +
                '<a href="/monitor/data.json">data.json</a>, so there is no sourced rate to apply. This ' +
                'is a placeholder, not a guess. The rate, when published, is the NiceHash SHA-256 price ' +
                'via ' + src('crypto51.app', LEASE_SRC) + '.'
        });
        setText('d-leasehour', 'not derived');
        setText('d-leaseday', 'not derived');
      }

      paintAvailability(calc.networkH, calc.rentable, fromCache);
    }

    /* The observed spacing and the difficulty are two views of the same thing;
       say so where they can be compared. */
    if (V.difficulty && V.spacingMean > 0) {
      var implied = V.difficulty * Math.pow(2, 32) / V.spacingMean;
      var sm = el('meta-bsv-spacing');
      if (sm) {
        sm.innerHTML += ' \u00b7 The difficulty also implies ' + hashText(implied) +
          ' at this observed spacing, against the difficulty-based estimate above \u2014 the two differ ' +
          'when blocks are arriving faster or slower than the 600 s target.';
      }
    }
  }

  /* ------------------------------------------------------------------ init */

  function refresh() {
    runState.total = 0;
    runState.live = 0;
    /* Clear the live values first: if this refresh fails, the derived cards must
       say they fell back to cache, not reuse a value from the previous refresh. */
    V.dataLoaded = false;
    V.assumptions = null;
    V.reserve = null;
    V.supply = null;
    V.hashrateH = null;
    V.difficulty = null;
    V.spacingMean = null;
    V.bsvPrice = null;
    var u = el('updated');
    if (u) u.textContent = 'Refreshing live figures\u2026';

    var jobs = [
      run(['bsv-height', 'bsv-hashrate'], fetchChain),
      run(['bsv-spacing'], fetchSpacing),
      run(['bsv-price', 'sol-price'], fetchPrices),
      run(['sol-stake'], fetchStake),
      run(['sol-tps'], fetchTps)
    ];

    jobs.push(loadData().then(function (j) {
      paintPublished(j);
    }, function (e) {
      V.dataLoaded = false;
      paintPublished({});
      setText('published-at', 'could not read /monitor/data.json (' + errMsg(e) +
        ') \u2014 open this page over http(s), not file://, and check the file is valid JSON');
    }));

    return Promise.all(jobs).then(function () {
      paintDerived();
      if (u) {
        var line;
        if (runState.live === runState.total) {
          line = 'Updated ' + clock() + ' \u00b7 all ' + runState.total + ' live figures read from their source.';
        } else if (runState.live > 0) {
          line = 'Updated ' + clock() + ' \u00b7 ' + runState.live + ' of ' + runState.total +
            ' live figures read from their source; the rest show a cached value or an explicit "unavailable".';
        } else {
          line = 'Updated ' + clock() + ' \u00b7 no source responded, so nothing on this page is being ' +
            'measured right now. Every figure below is marked cached or unavailable \u2014 none of them is a zero.';
        }
        u.textContent = line;
      }
    });
  }

  function start() {
    var btn = el('refresh');
    if (btn) btn.addEventListener('click', function () { refresh(); });
    refresh();
    /* Keep it current without hammering keyless endpoints, and stay quiet in a
       background tab. No animation is used, so reduced-motion changes nothing. */
    setInterval(function () {
      if (!document.hidden) refresh();
    }, 120000);
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', start);
  } else {
    start();
  }
})();
