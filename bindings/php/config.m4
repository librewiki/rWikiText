PHP_ARG_WITH([rwikitext], [for rWikiText support],
  [AS_HELP_STRING([--with-rwikitext=DIR], [repository root containing include/ and target/release/])], [no])

if test "$PHP_RWIKITEXT" != "no"; then
  if test ! -f "$PHP_RWIKITEXT/include/rwikitext.h" || test ! -f "$PHP_RWIKITEXT/target/release/librwikitext.a"; then
    AC_MSG_ERROR([rWikiText header or release static library not found in $PHP_RWIKITEXT])
  fi
  PHP_ADD_INCLUDE([$PHP_RWIKITEXT/include])
  RWIKITEXT_SHARED_LIBADD="$PHP_RWIKITEXT/target/release/librwikitext.a -ldl -lpthread -lm"
  PHP_SUBST([RWIKITEXT_SHARED_LIBADD])
  PHP_NEW_EXTENSION([rwikitext], [rwikitext.c], [$ext_shared])
fi
