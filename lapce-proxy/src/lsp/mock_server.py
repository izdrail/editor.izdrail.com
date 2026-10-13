import sys,json
files={}
def send(obj):
 b=json.dumps(obj).encode();sys.stdout.buffer.write(b'Content-Length: '+str(len(b)).encode()+b'\r\n\r\n'+b);sys.stdout.buffer.flush()
while True:
 headers={}
 while True:
  l=sys.stdin.buffer.readline()
  if not l:sys.exit(0)
  if l==b'\r\n':break
  k,v=l.decode().split(':',1);headers[k.lower()]=v.strip()
 msg=json.loads(sys.stdin.buffer.read(int(headers['content-length'])))
 m=msg.get('method');p=msg.get('params',{})
 if m=='initialize':send({'jsonrpc':'2.0','id':msg['id'],'result':{'capabilities':{'textDocumentSync':{'openClose':True,'change':2,'save':{'includeText':True}},'hoverProvider':True,'definitionProvider':True,'completionProvider':{'resolveProvider':True}}}})
 elif m=='textDocument/didOpen':
  files[p['textDocument']['uri']]=p['textDocument']['text'];send({'jsonrpc':'2.0','method':'textDocument/publishDiagnostics','params':{'uri':p['textDocument']['uri'],'diagnostics':[{'range':{'start':{'line':0,'character':0},'end':{'line':0,'character':1}},'message':'mock diagnostic'}]}})
 elif m=='textDocument/didChange':files[p['textDocument']['uri']]=p['contentChanges'][0]['text']
 elif m=='textDocument/hover':send({'jsonrpc':'2.0','id':msg['id'],'result':{'contents':{'kind':'plaintext','value':files[p['textDocument']['uri']]}}})
 elif m=='textDocument/completion':send({'jsonrpc':'2.0','id':msg['id'],'result':[{'label':'mock_completion'}]})
 elif m=='completionItem/resolve':p['detail']='resolved';send({'jsonrpc':'2.0','id':msg['id'],'result':p})
 elif m=='textDocument/definition':send({'jsonrpc':'2.0','id':msg['id'],'result':{'uri':p['textDocument']['uri'],'range':{'start':{'line':0,'character':0},'end':{'line':0,'character':1}}}})
 elif m=='shutdown':send({'jsonrpc':'2.0','id':msg['id'],'result':None})
 elif m=='exit':sys.exit(0)
 elif 'id' in msg:send({'jsonrpc':'2.0','id':msg['id'],'result':None})
