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

  /* The leased RATE is a published snapshot, not a live API call: the page
     reads leased_rate_usd_per_eh_hour from data.json, whose source field cites
     crypto51.app and the time it was read. This URL is the citation shown on
     the page. The rentable SUPPLY is different: it is fetched live from
     NiceHash, whose source URL, algo ids and market names all come from
     data.json (`rentable_markets`), never from this file. */
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
    rentableCfg: null,
    rentable: null,
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

  /* Rentable-market speeds arrive in H/s, so they are formatted by hashText()
     like every other hashrate rather than in a fixed unit: the three markets
     differ by four orders of magnitude, and forcing them into EH/s would round
     the legacy SHA256 market to a fake 0.00. */
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

    /* These two come from rentable_markets, not from attack_cost_assumptions,
       so they are set whatever happens to the latter. paintRentable() restates
       a-rentable once the live market read finishes. */
    var rc = V.rentable;
    setText('a-rentable', rc == null
      ? 'not read \u2014 see the market note above'
      : hashText(rc.totalH) + ' across ' + rc.rows.length + ' NiceHash markets (' +
        (rc.live ? 'live' : 'hand-published snapshot') + ')');
    var mm = numOrNull(V.rentableCfg && V.rentableCfg.majority_multiple);
    setText('a-majmult', mm == null
      ? 'not published in data.json'
      : dec(mm, 2) + ' \u00d7 the network estimate (published)');

    var ids = ['a-multiple', 'a-machine', 'a-mh', 'a-power', 'a-energy', 'a-lease-rate', 'a-target'];
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
      V.rentableCfg = j.rentable_markets || null;
      V.reserve = numOrNull(j.reserve && j.reserve.bsv_held);
      V.supply = numOrNull(j.supply && j.supply.solbsv_minted);
      return j;
    });
  }

  /* --- rentable SHA-256: the three NiceHash markets, summed ----------------- */

  /* The market that matters is SHA256AsicBoost (algo 35), where virtually all
     modern Bitcoin hardware lives — NOT the legacy SHA256 (algo 1) market that
     crypto51 reads. NiceHash splits SHA-256 across three markets and they must
     be summed: 35 (BTC), 2035 (USDT) and 1 (BTC). The algo ids, the names, the
     market labels and the source URL all come from data.json
     (`rentable_markets`); this function hard-codes none of them, and it matches
     on `a` (the algo id) rather than on the name, because 35 and 2035 are
     different markets with different price units.

     A missing algo id is treated as a FAILED read, never as a zero: summing the
     ones that answered would silently understate the rentable supply, which is
     the exact error this page exists to correct. On any failure the live read
     falls back to the hand-published snapshot in data.json, labelled as a
     snapshot and with the failure named; if there is no usable snapshot either,
     the caller renders an explicit placeholder. */
  function snapshotRentable(cfg, reason) {
    var markets = (cfg && cfg.markets) || [];
    var rows = markets.map(function (m) {
      return {
        id: numOrNull(m && m.algo_id),
        name: (m && m.name) || null,
        market: (m && m.market) || null,
        h: numOrNull(m && m.snapshot_h_per_s),
        live: false
      };
    });
    if (!rows.length || rows.some(function (r) { return r.id == null || r.h == null; })) return null;
    var total = rows.reduce(function (a, r) { return a + r.h; }, 0);
    if (!(total > 0)) return null;
    var at = cfg && cfg.snapshot_read_at ? Date.parse(cfg.snapshot_read_at) : NaN;
    return {
      rows: rows,
      totalH: total,
      live: false,
      at: isFinite(at) ? at : null,
      sourceName: (cfg && cfg.source_name) || null,
      sourceUrl: (cfg && cfg.source_url) || null,
      reason: reason || 'the live source could not be read'
    };
  }

  function fetchRentable(cfg) {
    if (!cfg) return Promise.resolve(null);
    var markets = cfg.markets;
    if (!markets || !markets.length) return Promise.resolve(null);
    var url = cfg.source_url;
    var live = url
      ? fetchJSON(url)
      : Promise.reject(new Error('no source_url published in data.json'));

    return live.then(function (j) {
      var speed = {};
      var feed = (j && j.algos) || [];
      feed.forEach(function (a) {
        var s = a ? numOrNull(a.s) : null;
        if (s != null) speed[a.a] = s;
      });
      var rows = markets.map(function (m) {
        var id = numOrNull(m && m.algo_id);
        var h = (id == null) ? null : speed[id];
        return {
          id: id, name: (m && m.name) || null, market: (m && m.market) || null,
          h: (h == null ? null : h), live: h != null
        };
      });
      var missing = rows.filter(function (r) { return !r.live; });
      if (missing.length) {
        throw new Error('the live response has no usable speed for algo ' +
          missing.map(function (r) { return (r.id == null ? '?' : r.id); }).join(', ') +
          ' \u2014 a partial sum would understate the market, so this read is treated as failed');
      }
      var total = rows.reduce(function (a, r) { return a + r.h; }, 0);
      if (!(total > 0)) throw new Error('the live response summed to a non-positive figure');
      return {
        rows: rows, totalH: total, live: true, at: Date.now(),
        sourceName: cfg.source_name || null, sourceUrl: url
      };
    }).catch(function (e) {
      /* .catch, not the second argument of .then: a missing algo throws from the
         handler above, and that has to reach the snapshot fallback too. */
      return snapshotRentable(cfg, errMsg(e));
    });
  }

  /* --- derived: ratio, attack cost, implied hashrate ----------------------- */

  /* Two independent bases, so a missing input on one does not blank the other:
       elec     — electricity-only floor from an assumed machine and energy price
       leased   — published SHA-256 market rate applied to the same hashrate
     `needH` is the attacker's hashrate (network estimate x multiple); at the
     default multiple of 1.0 it is the live network estimate itself. The
     rentable market is handled separately, from its own live source: see
     fetchRentable() and paintRentable(). */
  function computeAttack(hp, a) {
    if (!a) return null;
    var mult = numOrNull(a.hashrate_multiple);
    if (mult == null) mult = 1;
    var needH = hp * mult;
    var out = { mult: mult, networkH: hp, needH: needH, elec: null, leased: null };

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

    return out;
  }

  /* The rentable market, stated plainly. The consolidated figure is the sum of
     the three NiceHash SHA-256 markets (see fetchRentable); the ratio is
     rentable / BSV's LIVE network hashrate; the two out-hash costs are the
     published leased rate applied to the network (1x, which merely matches the
     honest chain) and to the majority multiple (2x). All of them are computed
     from live values — none is hard-coded.

     When the live read fails the block shows the hand-published snapshot from
     data.json, labelled as a snapshot and with the failure named. When there is
     no snapshot either it shows an explicit placeholder. It never shows a
     failed read as a zero, and it never presents the snapshot as live. */
  function paintRentable(hp, fromCache) {
    var cfg = V.rentableCfg;
    var r = V.rentable;
    var tbody = el('av-rows');
    var claim = el('av-claim');
    var state = el('av-state');

    var rate = numOrNull(V.assumptions && V.assumptions.leased_rate_usd_per_eh_hour);
    var majMult = numOrNull(cfg && cfg.majority_multiple);
    var ratio = (r && hp != null && hp > 0) ? (r.totalH / hp) : null;
    var algoIds = (cfg && cfg.markets)
      ? cfg.markets.map(function (m) { return String(m && m.algo_id); }).join(', ')
      : 'listed in data.json';

    setText('d-rentablemultiple', 'not derived');
    if (ratio != null) {
      setText('d-rentablemultiple',
        (ratio >= 10 ? NF.format(Math.round(ratio)) : dec(ratio, 2)) + '\u00d7 (rentable \u00f7 network)');
    }

    /* The same figure in the assumptions table. paintAssumptions() runs as soon
       as data.json arrives, before this read finishes, so restate it here. */
    setText('a-rentable', !cfg
      ? 'missing from data.json'
      : (r
        ? hashText(r.totalH) + ' across ' + r.rows.length + ' NiceHash markets (' +
          (r.live ? 'live' : 'hand-published snapshot') + ')'
        : 'not read \u2014 labelled placeholder, not 0'));

    /* --- the component rows, so the consolidation is visible -------------- */
    if (tbody) {
      if (!r) {
        tbody.innerHTML = '<tr><td colspan="2">' + (cfg
          ? 'No market reading is available, so there is nothing to sum. This is a placeholder, not a zero.'
          : 'The market list (algo ids, names, source) could not be read from ' +
            '<a href="/monitor/data.json">data.json</a>, so there is nothing to sum. This is a ' +
            'placeholder, not a zero.') + '</td></tr>';
      } else {
        var out = r.rows.map(function (m) {
          var label = esc(m.name || ('algo ' + m.id)) + ' <span class="av-sub">(algo ' +
            esc(String(m.id)) + ' \u00b7 ' + esc(m.market || '?') + ')</span>';
          return '<tr><td>' + label + '</td><td>' + esc(hashText(m.h)) + '</td></tr>';
        });
        out.push('<tr class="av-total"><td><strong>Consolidated &mdash; sum of the three</strong></td>' +
          '<td><strong>' + esc(hashText(r.totalH)) + '</strong></td></tr>');
        tbody.innerHTML = out.join('');
      }
    }

    /* --- the plain statement, with the live numbers ----------------------- */
    if (claim) {
      if (!r) {
        claim.innerHTML = 'The three NiceHash SHA-256 markets that make up the rentable supply could ' +
          'not be read' + (cfg ? ' (the live source failed and <a href="/monitor/data.json">data.json</a> ' +
          'holds no usable snapshot)' : '') + ', so the size of the rental market and the ratio ' +
          'against BSV\u2019s network are labelled placeholders right now \u2014 not zeros and not ' +
          'guesses.' + (rate > 0
            ? ' The out-hash costs below still come from the live network hashrate and the published ' +
              'leased rate.'
            : '');
      } else {
        var unit = r.live ? 'live' : 'snapshot';
        var parts = [];
        parts.push('The three NiceHash SHA-256 markets below list <strong class="av-num">' +
          esc(hashText(r.totalH)) + '</strong> between them (' + unit + ' read)');
        if (ratio != null) {
          parts.push(' \u2014 roughly <strong class="av-num">' +
            esc(ratio >= 10 ? NF.format(Math.round(ratio)) : dec(ratio, 2)) + '\u00d7</strong> the entire ' +
            'BSV network of <strong class="av-num">' + esc(hashText(hp)) + '</strong> (' +
            (fromCache ? 'last-read' : 'live') + ')');
        } else {
          parts.push('; BSV\u2019s network hashrate could not be read, so the ratio is a placeholder ' +
            'right now');
        }
        parts.push('.');
        if (ratio != null && rate > 0) {
          var matchDay = (hp / 1e18) * rate * 24;
          parts.push(' Matching that network at the published leased rate costs <strong class="av-num">' +
            esc(money(matchDay)) + '/day</strong>');
          if (majMult > 0) {
            parts.push('; a comfortable <strong class="av-num">' + esc(dec(majMult, 0)) +
              '\u00d7 majority</strong> costs <strong class="av-num">' +
              esc(money(matchDay * majMult)) + '/day</strong>');
          }
          parts.push('.');
        } else {
          parts.push(' The out-hash cost is a placeholder until both the live hashrate and the ' +
            'published leased rate are readable.');
        }
        claim.innerHTML = parts.join('');
      }
    }

    /* --- the caveat that survives: price impact, not availability ---------- */
    var impact = el('av-impact');
    if (impact && cfg && cfg.price_impact_source_url) {
      impact.innerHTML = '<a href="' + esc(cfg.price_impact_source_url) + '" target="_blank" ' +
        'rel="noopener">' + esc(cfg.price_impact_source_name || 'NiceHash hash-power marketplace') +
        '</a> guidance is that large demand moves the price of the order book, so a rental of this ' +
        'size is slower and costlier than the flat rate suggests. The hashrate is still there to ' +
        'rent; it is the price and the time that move.';
    }

    /* --- provenance: live, snapshot, or nothing --------------------------- */
    if (state) {
      var link = '<a href="' + esc(r && r.sourceUrl ? r.sourceUrl : '') + '" target="_blank" ' +
        'rel="noopener">' + esc((r && r.sourceName) || 'NiceHash public stats API') + '</a>';
      if (!r) {
        setText('av-state', 'No market reading \u2014 labelled placeholder, not 0.');
        state.className = 'mon-card__meta is-down';
      } else if (r.live) {
        state.className = 'mon-card__meta is-live';
        state.innerHTML = 'Live \u2014 read from ' + link +
          ' \u00b7 summed over the algo ids published in <a href="/monitor/data.json">data.json</a> (' +
          esc(algoIds) + ') \u00b7 read ' + clock() + '.';
      } else {
        state.className = 'mon-card__meta is-cached';
        state.innerHTML = 'The live read failed (' + esc(r.reason) + '). Showing the hand-published ' +
          'snapshot in <a href="/monitor/data.json">data.json</a>' +
          (r.at != null ? ', read ' + utc(r.at) + ' (' + age(r.at) + ' old)' : '') +
          ' \u2014 labelled a snapshot, not a live value. Source: ' + link + '.';
      }
    }
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
    var leaseRowIds = ['d-leasehour', 'd-leaseday'];
    var marketRowIds = ['d-rentablemultiple', 'd-matchday', 'd-majday'];

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
              '<a href="' + esc(LEASE_SRC) + '" target="_blank" rel="noopener">' +
              'crypto51.app (NiceHash SHA-256 prices)</a>.'
      });
      elecRowIds.concat(leaseRowIds, marketRowIds).forEach(function (id) { setText(id, 'not derived'); });
      paintRentable(null, false);
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
              'via <a href="' + esc(LEASE_SRC) + '" target="_blank" rel="noopener">crypto51.app</a>.'
      });
      elecRowIds.concat(leaseRowIds, marketRowIds).forEach(function (id) { setText(id, 'not derived'); });
      paintRentable(hp, fromCache);
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
                '. A rate, not an offer. The SHA-256 rental market is larger than BSV\u2019s whole ' +
                'network \u2014 see the market note above \u2014 so this cost is not capped by listed ' +
                'supply; what limits a real attack is price impact, not availability. Excludes buying ' +
                'the hardware.'
        });
        setText('d-leasehour', money(L.perHour));
        setText('d-leaseday', money(L.perDay));

        /* The out-hash cost per day, on the two bases actually asked for:
           1x the network (a bare match, a 50% share of the total) and the
           majority multiple from data.json (2x, about 67%). Both are the
           published leased rate applied to the LIVE network hashrate, so they
           follow the network instead of being hard-coded. */
        var majMult = numOrNull(V.rentableCfg && V.rentableCfg.majority_multiple);
        var matchDay = (hp / 1e18) * L.rate * 24;
        setText('d-matchday', money(matchDay) + '/day at 1\u00d7 the network');
        if (majMult > 0) {
          setText('d-majday', money(matchDay * majMult) + '/day at ' + dec(majMult, 0) +
            '\u00d7 the network');
        } else {
          setText('d-majday', 'majority_multiple not published in data.json');
        }
      } else {
        paint('lease', {
          text: 'NOT PUBLISHED', state: 'rate not published', stateClass: 'is-none', cache: false,
          meta: 'No usable leased_rate_usd_per_eh_hour was read from ' +
                '<a href="/monitor/data.json">data.json</a>, so there is no sourced rate to apply. This ' +
                'is a placeholder, not a guess. The rate, when published, is the NiceHash SHA-256 price ' +
                'via <a href="' + esc(LEASE_SRC) + '" target="_blank" rel="noopener">crypto51.app</a>.'
        });
        setText('d-leasehour', 'not derived');
        setText('d-leaseday', 'not derived');
      }

      paintRentable(calc.networkH, fromCache);
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
    V.rentableCfg = null;
    V.rentable = null;
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

    /* The rentable-market read needs the algo ids and the source URL, so it
       waits for data.json rather than hard-coding either. It counts as its own
       live figure. */
    var dataJob = loadData();

    jobs.push(dataJob.then(function (j) {
      paintPublished(j);
    }, function (e) {
      V.dataLoaded = false;
      paintPublished({});
      setText('published-at', 'could not read /monitor/data.json (' + errMsg(e) +
        ') \u2014 open this page over http(s), not file://, and check the file is valid JSON');
    }));

    jobs.push(dataJob.then(function () {
      runState.total++;
      return fetchRentable(V.rentableCfg).then(function (r) {
        V.rentable = r;
        if (r && r.live) runState.live++;
      });
    }, function () {
      /* data.json failed, so the market list is unknown; paintRentable() says so. */
      V.rentable = null;
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
