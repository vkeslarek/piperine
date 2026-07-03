typedef double RealNumber, *RealVector;
#define PI 3.14159265358979323846264338327950288419716939937511

typedef struct          /* Rectangular form of complex number. */
{   RealNumber Real;
    RealNumber Imag;
}   ComplexNumber;

/* Macro function that is equivalent to += operator for complex numbers. */
#define  cplxADD_ASSIGN(to,from)        \
{   (to).Real += (from).Real;           \
    (to).Imag += (from).Imag;           \
}

/* Macro function that multiplies two complex numbers. */
#define  cplxMULT(to,from_a,from_b)             \
{   (to).Real = (from_a).Real * (from_b).Real - \
                (from_a).Imag * (from_b).Imag;  \
    (to).Imag = (from_a).Real * (from_b).Imag + \
                (from_a).Imag * (from_b).Real;  \
}

/* Macro function that implements to *= from for complex numbers. */
#define  cplxMULT_ASSIGN(to,from)               \
{   RealNumber to_real_ = (to).Real;            \
    (to).Real = to_real_ * (from).Real -        \
                (to).Imag * (from).Imag;        \
    (to).Imag = to_real_ * (from).Imag +        \
                (to).Imag * (from).Real;        \
}

/* Complex division:  to = num / den */
#define  cplxDIV(to,num,den)                                            \
{   RealNumber  r_, s_;                                                 \
    if (((den).Real >= (den).Imag && (den).Real > -(den).Imag) ||      \
        ((den).Real < (den).Imag && (den).Real <= -(den).Imag))        \
    {   r_ = (den).Imag / (den).Real;                                   \
        s_ = (den).Real + r_*(den).Imag;                                \
        (to).Real = ((num).Real + r_*(num).Imag)/s_;                    \
        (to).Imag = ((num).Imag - r_*(num).Real)/s_;                    \
    }                                                                   \
    else                                                                \
    {   r_ = (den).Real / (den).Imag;                                   \
        s_ = (den).Imag + r_*(den).Real;                                \
        (to).Real = (r_*(num).Real + (num).Imag)/s_;                    \
        (to).Imag = (r_*(num).Imag - (num).Real)/s_;                    \
    }                                                                   \
}

/* Complex division and assignment:  num /= den */
#define  cplxDIV_ASSIGN(num,den)                                        \
{   RealNumber  r_, s_, t_;                                             \
    if (((den).Real >= (den).Imag && (den).Real > -(den).Imag) ||      \
        ((den).Real < (den).Imag && (den).Real <= -(den).Imag))        \
    {   r_ = (den).Imag / (den).Real;                                   \
        s_ = (den).Real + r_*(den).Imag;                                \
        t_ = ((num).Real + r_*(num).Imag)/s_;                           \
        (num).Imag = ((num).Imag - r_*(num).Real)/s_;                   \
        (num).Real = t_;                                                \
    }                                                                   \
    else                                                                \
    {   r_ = (den).Real / (den).Imag;                                   \
        s_ = (den).Imag + r_*(den).Real;                                \
        t_ = (r_*(num).Real + (num).Imag)/s_;                           \
        (num).Imag = (r_*(num).Imag - (num).Real)/s_;                   \
        (num).Real = t_;                                                \
    }                                                                   \
}

/* Complex reciprocation:  to = 1.0 / den */
#define  cplxRECIPROCAL(to,den)                                         \
{   RealNumber  r_;                                                     \
    if (((den).Real >= (den).Imag && (den).Real > -(den).Imag) ||      \
        ((den).Real < (den).Imag && (den).Real <= -(den).Imag))        \
    {   r_ = (den).Imag / (den).Real;                                   \
        (to).Imag = -r_*((to).Real = 1.0/((den).Real + r_*(den).Imag)); \
    }                                                                   \
    else                                                                \
    {   r_ = (den).Real / (den).Imag;                                   \
        (to).Real = -r_*((to).Imag = -1.0/((den).Imag + r_*(den).Real));\
    }                                                                   \
}

#define  cplxASSIGN(to,from)    \
{   (to).Real = (from).Real;    \
    (to).Imag = (from).Imag;    \
}

#define  sclrASSIGN(to,re,im)	\
{   (to).Real = (re);		\
    (to).Imag = (im);		\
}

/* Macro function that multiplies a complex number by a scalar. */
#define  sclrMULT(to,sclr,cplx) \
{   (to).Real = (sclr) * (cplx).Real;   \
    (to).Imag = (sclr) * (cplx).Imag;   \
}

/* Macro function that multiply-assigns a complex number by a scalar. */
#define  sclrMULT_ASSIGN(to,sclr)       \
{   (to).Real *= (sclr);                \
    (to).Imag *= (sclr);                \
}

/* Macro function that returns the magnitude (L-2 norm) of a complex number. */
#define  cplx2_NORM(a)          (hypot((a).Real, (a).Imag))

