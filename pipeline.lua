init = function(args)
    local r = {}
    r[1] = wrk.format(nil, "/res/loader?q=12")
    r[2] = wrk.format(nil, "/res/loader?q=13")
    r[3] = wrk.format(nil, "/res/loader?q=14")
    r[4] = wrk.format(nil, "/res/loader?q=15")
    r[5] = wrk.format(nil, "/res/loader?q=16")
    r[6] = wrk.format(nil, "/res/loader?q=17")
    r[7] = wrk.format(nil, "/res/loader?q=18")
    r[8] = wrk.format(nil, "/res/loader?q=19")
    r[9] = wrk.format(nil, "/res/loader?q=20")
 
    req = table.concat(r)
    print(req)
 end
 
 request = function()
    return req
 end
 