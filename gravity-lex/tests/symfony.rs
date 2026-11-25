use gravity_lex::{lex,style,symfony};
fn output(s:&str)->Vec<u8>{let mut o=lex(s);assert!(symfony::overlay(s,&mut o.styles,"Request Response Route Autowire"));o.styles}
fn scope(s:&str,n:&str,id:u8){let at=s.find(n).unwrap();let o=output(s);assert!(o[at..at+n.len()].iter().all(|v|*v==id),"{n}: {:?}",&o[at..at+n.len()]);}
#[test]
fn routing_controller_and_di_attributes(){
 let s="<?php #[Route('/x', name: 'x')] #[AsController] class Home extends AbstractController { public function __invoke(#[Autowire(service: 'x')] Request $request): Response { return $this->render('x'); } }";
 for word in ["Route","AsController","Autowire"]{scope(s,word,symfony::ATTRIBUTE)}
 for word in ["AbstractController","Request $request","Response {"]{let at=s.find(word).unwrap();assert_eq!(output(s)[at],symfony::TYPE);}
 scope(s,"render",symfony::METHOD);scope(s,"'/x'",style::SINGLE);
}
#[test]
fn console_events_messenger_forms_validation_and_doctrine(){
 let s="<?php #[AsCommand(name:'app:send')] class Send extends Command implements EventSubscriberInterface { public function execute(InputInterface $in, OutputInterface $out) {} } #[AsMessageHandler] class Handler {} #[ORM\\Entity] class Item { #[ORM\\Id, ORM\\GeneratedValue, ORM\\Column] #[Assert\\NotBlank] public string $name; #[ORM\\ManyToOne] public $owner; } class Form extends AbstractType { public function buildForm(FormBuilderInterface $builder) { $builder->add('name'); $em->persist($item); $em->flush(); $bus->dispatch($message); } }";
 for n in ["AsCommand","AsMessageHandler","Entity","GeneratedValue","Column","NotBlank","ManyToOne"]{scope(s,n,symfony::ATTRIBUTE)}
 for n in ["Command implements","EventSubscriberInterface","AbstractType","FormBuilderInterface"]{let at=s.find(n).unwrap();assert_eq!(output(s)[at],symfony::TYPE);}
 for n in ["persist","flush","dispatch"]{scope(s,n,symfony::METHOD)}
}
#[test]
fn comments_strings_host_and_unrelated_words_stay_base(){
 let s="<p>Route render</p><?php $s='#[Route]'; /* #[Entity] */ function render() {} $Route=1; $a=['Route'];";
 scope(s,"<",style::HTML_DELIMITER);scope(s,"'#[Route]'",style::SINGLE);scope(s,"/* #[Entity] */",style::COMMENT);scope(s,"$Route",style::VARIABLE);
 let o=output(s);let at=s.find("function render").unwrap()+9;assert_ne!(o[at],symfony::METHOD);let at=s.find("'Route'").unwrap();assert_eq!(o[at],style::SINGLE);
}
#[test]
fn utf8_prefixes_and_unfinished_attributes_are_total(){
 let s="<?php #[Route('/λ', defaults: ['x'=>fn()=>[1,2]])] class X extends AbstractController {";
 for(end,_)in s.char_indices().chain(std::iter::once((s.len(),'\0'))){let o=output(&s[..end]);assert_eq!(o.len(),end);for(at,c)in s[..end].char_indices(){assert!(o[at..at+c.len_utf8()].iter().all(|v|*v==o[at]));}}
 assert!(!symfony::overlay("λ",&mut[0],""));
}
