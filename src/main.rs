mod parser;

use parser::*;
fn main() -> Result<(), parser::ParsingError> {
    let packet = "POST /cgi-bin/process.cgi HTTP/1.1\r
User-Agent: Mozilla/4.0 (compatible; MSIE5.01; Windows NT)\r
Host: www.tutorialspoint.com\r
Content-Type: application/x-www-form-urlencoded\r
Content-Length: length\r
Accept-Language: en-us\r
Accept-Encoding: gzip, deflate\r
Connection: Keep-Alive\r
\r
licenseID=string&content=string&/paramsXML=string";
    let parser = HttpRequestParser::<RequestLine<Method>>::start(packet);
    println!("{:#?}", parser);
    let parser = parser.parse()?;
    println!("{:#?}", parser);
    let parser = parser.parse()?;
    println!("{:#?}", parser);
    let parser = parser.parse()?;
    println!("{:#?}", parser);
    let parser = parser.parse()?;
    println!("{:#?}", parser);
    let request = parser.parse();
    println!("{:#?}", request);
    Ok(())
}
