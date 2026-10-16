# Go to Symfony parameter

Put the cursor on a parameter and run Go to Definition, in a YAML, XML or PHP file:

    arguments: ['%app.admin_email%']                     # inside %...%
    $this->getParameter('app.admin_email')               # also hasParameter(...)
    #[Autowire(param: 'app.admin_email')]
    #[Autowire('%app.admin_email%')]

It opens `app.admin_email:` under the top-level `parameters:` key in `config/services*.yaml` or
`config/packages/**.yaml`, or `<parameter key="app.admin_email">` in `config/services*.xml`.

Only keys under `parameters:` count, so a service with the same id is not matched. Not found:
built-in parameters (`kernel.project_dir`), `%env(...)%` (use Go to Env), parameters set from PHP,
`when@env` blocks nested inside other keys. Limits: 80 config files, 1 MB per file. Nothing is
executed. If nothing is found, normal Go to Definition runs.
