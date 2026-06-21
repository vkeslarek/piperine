#                                               -*- Autoconf -*-
# Process this file with autoconf to produce a configure script.
# configure.ac for ngspice-doc

# Copyright (C) 2026 Holger Vogt
#           (C) 2018 Carsten Schoenert <c.schoenert@...>
#
# This file is part of the ngspice-doc. Please see README and COPYING for
# terms of use.
#
# SPDX-License-Identifier:        CC-BY-SA-4.0

# To regenerate the configure script call
#    autoreconf  -vi

AC_INIT(ngspice-doc,46,[http://ngspice.sourceforge.net/bugrep.html])
AC_PREREQ(2.61)

AC_CONFIG_AUX_DIR(config)

AM_INIT_AUTOMAKE

# The minimum required LyX version for building the documentation.
# This is the LyX version the file manual.lyx was created with.
lyx_version_required=2.4

AC_CHECK_PROG([LYX],
              lyx,
              [yes],
              [no],
              [$PATH])

if test "x$LYX" = "xyes"; then
    # There is no easy way to detect the installed version of lyx.
    lyx_version=`lyx -version | head -1 | awk '{print $2}'`

    AS_VERSION_COMPARE([$lyx_version],
                       [$lyx_version_required],
                       [AC_MSG_ERROR([Lyx version $lyx_version found, but at least $lyx_version_required needed])]
                       [AC_MSG_RESULT([yes $lyx_version])],
                       [AC_MSG_RESULT([yes $lyx_version])])
else
   echo
   AC_MSG_ERROR([The LyX package was not found!])
fi

AC_ARG_WITH([lyxuserdir],
            [AS_HELP_STRING([--with-lyxuserdir=DIR],
                            [use a dedicated Lyx user directory])],
            [with_lyxuserdir="$withval"],
            [with_lyxuserdir="no"])

# If the user has given the option '--with-lyxuserdir' substitute the given
# argument into the Makefile.
if test "x$with_lyxuserdir" != "xno"; then
    LYX_USER_DIR="-userdir $with_lyxuserdir"
    AC_MSG_NOTICE([==> LYX_USER_DIR is configured to '$with_lyxuserdir'.])
else
    LYX_USER_DIR=""
fi

AC_SUBST([LYX_USER_DIR])

AC_CONFIG_FILES([Makefile])
AC_OUTPUT
