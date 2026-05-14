; Comments
(comment) @comment
(documentation_comment) @comment

; Class definitions
(class_definition
  name: (identifier) @definition.class)

; Enum definitions
(enum_declaration
  name: (identifier) @definition.class)

; Function definitions
(function_signature
  name: (identifier) @definition.def)

(getter_signature
  (identifier) @definition.def)

(setter_signature
  name: (identifier) @definition.def)

; Constructor definitions
(constructor_signature
  name: (identifier) @definition.constructor)
