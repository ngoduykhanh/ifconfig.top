import geoip2.database


with geoip2.database.Reader('./GeoLite2-City.mmdb') as reader:
    response = reader.city('101.231.101.116')
    print(response.country.names['zh-CN'])
    print(response.city.names['zh-CN'])
    print(response.raw)