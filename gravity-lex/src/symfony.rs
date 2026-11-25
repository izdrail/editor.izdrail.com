//! Symfony/Doctrine lexical conventions layered on existing PHP tokens.
//! Project detection and version-matched metadata are supplied by the caller.
use crate::{style,laravel::{tokens_cancellable,code_identifier}};
pub const TYPE:u8=45;
pub const ATTRIBUTE:u8=46;
pub const METHOD:u8=47;
fn member(word:&str,list:&str)->bool{list.split_ascii_whitespace().any(|value|value==word)}

pub fn overlay(source:&str,styles:&mut[u8],types:&str)->bool {overlay_cancellable(source,styles,types,None)}
pub fn overlay_cancellable(source:&str,styles:&mut[u8],types:&str,cancel:Option<&std::sync::atomic::AtomicUsize>)->bool {
 if source.len()!=styles.len(){return false;}
 let Some(tokens)=tokens_cancellable(source,styles,cancel)else{return false;};
 let text=|i:usize| &source[tokens[i].range.clone()];
 let attributes="Route AsController IsGranted MapEntity MapRequestPayload MapQueryParameter MapQueryString CurrentUser Autowire AutowireIterator AutowireLocator Autoconfigure AutoconfigureTag AsAlias AsDecorator TaggedIterator TaggedLocator Target Required Exclude When WhenNot AsCommand AsEventListener AsMessageHandler AsSchedule AsCronTask AsPeriodicTask Entity Table Column Id GeneratedValue ManyToOne OneToMany ManyToMany OneToOne JoinColumn JoinTable Index UniqueConstraint Embeddable Embedded HasLifecycleCallbacks PrePersist PostPersist PreUpdate PostUpdate PreRemove PostRemove PreFlush PostLoad NotBlank NotNull Blank IsNull Valid Length Range Choice Email Url Regex Positive PositiveOrZero Negative NegativeOrZero GreaterThan GreaterThanOrEqual LessThan LessThanOrEqual EqualTo NotEqualTo Type Count Collection All Sequentially Callback Expression Unique File Image Date DateTime Time Uuid Ulid";
 let conventions="AbstractController Controller Request Response JsonResponse RedirectResponse StreamedResponse RequestStack ParameterBagInterface ContainerInterface ServiceSubscriberInterface ServiceSubscriberTrait KernelInterface Command InputInterface OutputInterface SymfonyStyle EventDispatcherInterface EventSubscriberInterface MessageBusInterface Envelope HandlerArgumentsStamp AbstractType FormBuilderInterface FormInterface OptionsResolver Constraint ConstraintValidator ValidatorInterface EntityManagerInterface EntityRepository ServiceEntityRepository ManagerRegistry Collection ArrayCollection";
 let methods="render renderView redirectToRoute generateUrl json createNotFoundException denyAccessUnlessGranted isGranted getUser addFlash createForm handleRequest isSubmitted isValid getData setDefaults configureOptions buildForm configure execute interact initialize getSubscribedEvents dispatch addSubscriber addListener handle find findAll findBy findOneBy getRepository persist flush remove createQueryBuilder join leftJoin innerJoin andWhere orWhere setParameter getQuery getResult getOneOrNullResult getSingleScalarResult addSelect orderBy setMaxResults getDoctrine getParameter hasParameter autowire autoconfigure tag args arg bind set alias import load defaults public private lazy decorate";
 let mut attribute_depth=0usize;
 let mut expecting_attribute=false;
 for(index,token)in tokens.iter().enumerate(){
  if index%1024==0 {std::thread::yield_now();if cancel.is_some_and(|flag|flag.load(std::sync::atomic::Ordering::Relaxed)!=0){return false;}}
  let word=text(index);
  if token.kind==style::OPERATOR {
   if word=="#" && index+1<tokens.len() && text(index+1)=="[" {expecting_attribute=true;}
   if word=="[" {if expecting_attribute || attribute_depth>0 {attribute_depth+=1;}}
   if word=="]" {attribute_depth=attribute_depth.saturating_sub(1);if attribute_depth==0 {expecting_attribute=false;}}
   continue;
  }
  if !code_identifier(token.kind){continue;}
  let prev=index.checked_sub(1).map(text).unwrap_or("");
  let next=if index+1<tokens.len(){text(index+1)}else{""};
  let attribute=attribute_depth>0 && member(word,attributes) && matches!(prev,"["|","|"\\") && matches!(next,"("|"]"|",");
  let type_context=matches!(prev,"extends"|"implements"|"use"|"new"|"\\"|"|"|"?"|":"|",") || next=="::" || (next.starts_with('$') && tokens.get(index+1).is_some_and(|t|t.kind==style::VARIABLE));
  let scope=if attribute {Some(ATTRIBUTE)}
   else if type_context && (member(word,types)||member(word,conventions)) {Some(TYPE)}
   else if matches!(prev,"->"|"?->"|"::") && next=="(" && member(word,methods) {Some(METHOD)}
   else{None};
  if let Some(scope)=scope{styles[token.range.clone()].fill(scope);}
 }
 true
}
