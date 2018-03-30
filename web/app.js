var fs = require('fs');
var http = require('http');
var path = require('path');
var url = require('url');

var PORT = 8080;
var DATA_DIR = '/opt/data/ncpr';

var HEAD_MAP = {};
var files = fs.readdirSync(DATA_DIR).filter(function(x){return x.match(/^\d{1,4}\.dat/);});
for (var i = 0; i < files.length; i++) {
    HEAD_MAP[+files[i].split('.')[0]] = fs.readFileSync(path.join(files[i]));
}

function deserialize(b1, b2) {
    if ((b1 & 0x80) === 0) {
        return null;
    }

    return [
        (b1 & 0x7c) >> 2,
        ((b2 & 0xfe) === 0) ? "0" : [1, 2, 3, 4, 5, 6, 7].filter(function(x){return ((b2 >> x) & 1) === 1;}).join("#"),
        ((b2 & 1) === 0) ? "D" : "A",
        b1 & 0x03
    ];
}

function binarySearch(key, data, low, high) {
    if (low > high) {
        return null;
    }

    var mid = (low + high) >> 1;
    var midKey = (data[mid * 5 + 1] << 16) | (data[mid * 5 + 2] << 8) | data[mid * 5 + 3];

    if (key < midKey) {
        return binarySearch(key, data, low, mid - 1);
    } else if (key === midKey) {
        return deserialize(data[mid * 5 + 4], data[mid * 5 + 5]);
    } else {
        return binarySearch(key, data, mid + 1, high);
    }
}

function getNumberInfo(number) {
    var h = +number.substring(0, 4);
    var t = +number.substring(4);

    if (!HEAD_MAP.hasOwnProperty(h)) {
        return null;
    }

    var buffer = HEAD_MAP[h];
    if (buffer[0] === 0) {
        return deserialize(buffer[2 * t + 1], buffer[2 * t + 2]);
    } else {
        return binarySearch(t, buffer, 0, (buffer.length - 1) / 5 - 1);
    }
}

var server = http.createServer(function(req, res) {
    var parsedUrl = url.parse(req.url, true);
    if (req.method !== 'GET' || parsedUrl.pathname !== '/api/v1/status' || !parsedUrl.query.hasOwnProperty('numbers')) {
        return res.end('{}');
    }

    var result = {};
    var query = parsedUrl.query['numbers'];  // multiple query string keys
    var numbers = (typeof query === 'string') ? query.split(',') : query;
    for (var i = 0; i < numbers.length; i++) {
        if (numbers[i].match('^\\d{10}$') === null) {
            continue;
        }
        result[numbers[i]] = getNumberInfo(numbers[i]);
    }
    return res.end(JSON.stringify(result));
});

server.listen(PORT, function() {
    console.log("started listening on port " + PORT);
});
